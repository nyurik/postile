use mlt_core::encoder::EncoderConfig;
use mlt_core::geo_types::{
    Coord, Geometry, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon,
};
use mlt_core::{PropKind, PropValue, TileLayer};
use pgrx::{PgRelation, Spi, default, error, name_data_to_str, pg_extern, pg_sys};

use crate::compression;

#[pg_extern(immutable, parallel_safe)]
fn pt_gzip(data: Option<&[u8]>, level: default!(Option<i32>, "NULL")) -> Option<Vec<u8>> {
    // Need to take and return `Option` to handle NULL input in the second param
    // Otherwise calling it with NULL will panic, at least in tests
    data.map(|v| {
        compression::pt_gzip(v, level)
            .unwrap_or_else(|e| error!("pt_gzip failed: {}", e.to_string()))
    })
}

#[pg_extern(immutable, parallel_safe)]
fn pt_brotli(data: &[u8]) -> Vec<u8> {
    compression::pt_brotli(data).unwrap_or_else(|e| error!("pt_brotli failed: {}", e.to_string()))
}

#[derive(Clone)]
struct ColumnInfo {
    ordinal: usize,
    name: String,
    oid: pg_sys::Oid,
    kind: Option<PropKind>,
}

struct SelectedColumns {
    select_expressions: Vec<String>,
    property_columns: Vec<ColumnInfo>,
    feature_id_column: Option<ColumnInfo>,
    geometry_ordinal: usize,
}

struct MltFeatureRow {
    id: Option<u64>,
    geometry: Geometry<i32>,
    properties: Vec<PropValue>,
}

#[derive(Clone, Copy)]
enum GeometryColumnKind {
    PgPoint,
    Ewkb,
}

#[allow(clippy::needless_pass_by_value)]
#[pg_extern]
fn pt_asmlt(
    table_name: PgRelation,
    name: default!(String, "'default'"),
    extent: default!(i32, "4096"),
    geom_name: default!(String, "'geom'"),
    feature_id_name: default!(Option<String>, "NULL"),
) -> Vec<u8> {
    let extent = u32::try_from(extent)
        .unwrap_or_else(|_| error!("PT_AsMLT extent must be a non-negative integer"));
    let columns = table_columns(&table_name, &geom_name);
    let geom_column = columns
        .iter()
        .find(|column| column.name == geom_name)
        .expect("table_columns validates geometry column exists");
    let geometry_kind = if geom_column.oid == pg_sys::POINTOID {
        GeometryColumnKind::PgPoint
    } else {
        GeometryColumnKind::Ewkb
    };
    let geometry_select = match geometry_kind {
        GeometryColumnKind::PgPoint => format!("t.{} AS __pt_geom", quote_identifier(&geom_name)),
        GeometryColumnKind::Ewkb => {
            format!("ST_AsEWKB(t.{}) AS __pt_geom", quote_identifier(&geom_name))
        }
    };
    let selected_columns = select_columns(
        &columns,
        &geom_name,
        feature_id_name.as_deref(),
        geometry_select,
    );

    let select_list = selected_columns.select_expressions.join(", ");
    let table_name = quote_qualified_identifier(table_name.namespace(), table_name.name());
    let query = format!(
        "SELECT {select_list} FROM {table_name} AS t WHERE t.{} IS NOT NULL",
        quote_identifier(&geom_name)
    );

    let mut layer = TileLayer::builder(name, extent)
        .unwrap_or_else(|e| error!("PT_AsMLT failed to create layer: {e}"));
    let property_keys = selected_columns
        .property_columns
        .iter()
        .map(|column| {
            layer
                .add_property(
                    column.name.clone(),
                    column.kind.expect("property columns have scalar kinds"),
                )
                .unwrap_or_else(|e| error!("PT_AsMLT failed to add property: {e}"))
        })
        .collect::<Vec<_>>();

    for row in read_mlt_features(
        &query,
        selected_columns.geometry_ordinal,
        geometry_kind,
        &selected_columns.property_columns,
        selected_columns.feature_id_column.as_ref(),
    )
    .unwrap_or_else(|e| error!("PT_AsMLT failed: {e}"))
    {
        let mut feature = layer.feature(row.geometry);
        feature.id(row.id);
        for (key, property) in property_keys.iter().zip(row.properties) {
            feature
                .property(*key, property)
                .unwrap_or_else(|e| error!("PT_AsMLT failed to set property: {e}"));
        }
        feature
            .finish()
            .unwrap_or_else(|e| error!("PT_AsMLT failed to add feature: {e}"));
    }

    layer
        .finish()
        .encode(EncoderConfig::default())
        .unwrap_or_else(|e| error!("PT_AsMLT failed: {e}"))
}

fn select_columns(
    columns: &[ColumnInfo],
    geom_name: &str,
    feature_id_name: Option<&str>,
    geometry_select: String,
) -> SelectedColumns {
    let feature_id_column = feature_id_name.map(|feature_id_name| {
        let column = columns
            .iter()
            .find(|column| column.name == feature_id_name)
            .unwrap_or_else(|| {
                error!("PT_AsMLT feature_id_name {feature_id_name:?} does not exist")
            });
        if !matches!(
            column.kind,
            Some(PropKind::I32 | PropKind::U32 | PropKind::I64)
        ) {
            error!("PT_AsMLT feature_id_name must refer to an integer column");
        }
        column
    });

    let property_columns = columns
        .iter()
        .filter(|column| {
            column.name != geom_name
                && feature_id_name != Some(column.name.as_str())
                && column.kind.is_some()
        })
        .collect::<Vec<_>>();

    let mut select_expressions =
        Vec::with_capacity(property_columns.len() + usize::from(feature_id_column.is_some()) + 1);
    let mut next_ordinal = 1;
    let feature_id_column = feature_id_column.map(|column| {
        select_expressions.push(format!("t.{}", quote_identifier(&column.name)));
        let mut column = column.clone();
        column.ordinal = next_ordinal;
        next_ordinal += 1;
        column
    });
    let property_columns = property_columns
        .iter()
        .map(|column| {
            select_expressions.push(format!("t.{}", quote_identifier(&column.name)));
            let mut column = (*column).clone();
            column.ordinal = next_ordinal;
            next_ordinal += 1;
            column
        })
        .collect::<Vec<_>>();
    let geometry_ordinal = next_ordinal;
    select_expressions.push(geometry_select);

    SelectedColumns {
        select_expressions,
        property_columns,
        feature_id_column,
        geometry_ordinal,
    }
}

fn read_mlt_features(
    query: &str,
    geometry_ordinal: usize,
    geometry_kind: GeometryColumnKind,
    property_columns: &[ColumnInfo],
    feature_id_column: Option<&ColumnInfo>,
) -> Result<Vec<MltFeatureRow>, pgrx::spi::Error> {
    Spi::connect(|client| {
        let tuples = client.select(query, None, &[])?;
        let mut features = Vec::with_capacity(tuples.len());
        for tuple in tuples {
            let geometry = read_geometry(&tuple, geometry_ordinal, geometry_kind)?;
            let id = feature_id_column.and_then(|column| read_feature_id(&tuple, column));
            let properties = property_columns
                .iter()
                .map(|column| read_property(&tuple, column))
                .collect::<Result<Vec<_>, _>>()?;
            features.push(MltFeatureRow {
                id,
                geometry,
                properties,
            });
        }
        Ok(features)
    })
}

fn table_columns(table: &PgRelation, geom_name: &str) -> Vec<ColumnInfo> {
    let mut found_geometry = false;
    let columns = table
        .tuple_desc()
        .iter()
        .filter(|att| !att.attisdropped)
        .enumerate()
        .map(|(index, att)| {
            let name = name_data_to_str(&att.attname).to_string();
            if name == geom_name {
                found_geometry = true;
            }
            ColumnInfo {
                ordinal: index + 1,
                name,
                oid: att.atttypid,
                kind: scalar_prop_kind(att.atttypid),
            }
        })
        .collect::<Vec<_>>();

    if !found_geometry {
        error!("PT_AsMLT geometry column {geom_name:?} does not exist");
    }
    columns
}

fn scalar_prop_kind(oid: pg_sys::Oid) -> Option<PropKind> {
    match oid {
        pg_sys::BOOLOID => Some(PropKind::Bool),
        pg_sys::INT2OID | pg_sys::INT4OID => Some(PropKind::I32),
        pg_sys::OIDOID => Some(PropKind::U32),
        pg_sys::INT8OID => Some(PropKind::I64),
        pg_sys::FLOAT4OID => Some(PropKind::F32),
        pg_sys::FLOAT8OID => Some(PropKind::F64),
        pg_sys::TEXTOID | pg_sys::VARCHAROID | pg_sys::BPCHAROID => Some(PropKind::Str),
        _ => None,
    }
}

fn read_feature_id(tuple: &pgrx::spi::SpiHeapTupleData<'_>, column: &ColumnInfo) -> Option<u64> {
    match column.oid {
        pg_sys::INT2OID => tuple
            .get::<i16>(column.ordinal)
            .unwrap_or_else(|e| error!("PT_AsMLT failed to read feature id: {e}"))
            .and_then(|value| u64::try_from(value).ok()),
        pg_sys::INT4OID => tuple
            .get::<i32>(column.ordinal)
            .unwrap_or_else(|e| error!("PT_AsMLT failed to read feature id: {e}"))
            .and_then(|value| u64::try_from(value).ok()),
        pg_sys::OIDOID => tuple
            .get::<pg_sys::Oid>(column.ordinal)
            .unwrap_or_else(|e| error!("PT_AsMLT failed to read feature id: {e}"))
            .map(|value| u64::from(value.to_u32())),
        pg_sys::INT8OID => tuple
            .get::<i64>(column.ordinal)
            .unwrap_or_else(|e| error!("PT_AsMLT failed to read feature id: {e}"))
            .and_then(|value| u64::try_from(value).ok()),
        _ => error!("PT_AsMLT feature_id_name must refer to an integer column"),
    }
}

fn read_property(
    tuple: &pgrx::spi::SpiHeapTupleData<'_>,
    column: &ColumnInfo,
) -> Result<PropValue, pgrx::spi::Error> {
    Ok(match column.oid {
        pg_sys::BOOLOID => PropValue::Bool(tuple.get::<bool>(column.ordinal)?),
        pg_sys::INT2OID => PropValue::I32(tuple.get::<i16>(column.ordinal)?.map(i32::from)),
        pg_sys::INT4OID => PropValue::I32(tuple.get::<i32>(column.ordinal)?),
        pg_sys::OIDOID => PropValue::U32(
            tuple
                .get::<pg_sys::Oid>(column.ordinal)?
                .map(pg_sys::Oid::to_u32),
        ),
        pg_sys::INT8OID => PropValue::I64(tuple.get::<i64>(column.ordinal)?),
        pg_sys::FLOAT4OID => PropValue::F32(tuple.get::<f32>(column.ordinal)?),
        pg_sys::FLOAT8OID => PropValue::F64(tuple.get::<f64>(column.ordinal)?),
        pg_sys::TEXTOID | pg_sys::VARCHAROID | pg_sys::BPCHAROID => {
            PropValue::Str(tuple.get::<String>(column.ordinal)?)
        }
        _ => unreachable!("property columns have scalar kinds"),
    })
}

fn read_geometry(
    tuple: &pgrx::spi::SpiHeapTupleData<'_>,
    ordinal: usize,
    kind: GeometryColumnKind,
) -> Result<Geometry<i32>, pgrx::spi::Error> {
    Ok(match kind {
        GeometryColumnKind::PgPoint => {
            let point = tuple
                .get::<pg_sys::Point>(ordinal)?
                .unwrap_or_else(|| error!("PT_AsMLT geometry column cannot be NULL"));
            Geometry::Point(Point::new(tile_coord(point.x), tile_coord(point.y)))
        }
        GeometryColumnKind::Ewkb => {
            let ewkb = tuple
                .get::<&[u8]>(ordinal)?
                .unwrap_or_else(|| error!("PT_AsMLT geometry column cannot be NULL"));
            parse_ewkb_geometry(ewkb)
        }
    })
}

fn parse_ewkb_geometry(bytes: &[u8]) -> Geometry<i32> {
    EwkbReader::new(bytes)
        .read_geometry()
        .unwrap_or_else(|e| error!("PT_AsMLT failed to parse EWKB geometry: {e}"))
}

struct EwkbReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

struct EwkbHeader {
    big_endian: bool,
    geometry_type: u32,
    has_z: bool,
    has_m: bool,
}

impl<'a> EwkbReader<'a> {
    const Z_FLAG: u32 = 0x8000_0000;
    const M_FLAG: u32 = 0x4000_0000;
    const SRID_FLAG: u32 = 0x2000_0000;

    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_geometry(&mut self) -> Result<Geometry<i32>, String> {
        let header = self.read_header()?;
        match header.geometry_type {
            1 => Ok(Geometry::Point(self.read_point(&header)?)),
            2 => Ok(Geometry::LineString(self.read_line_string(&header)?)),
            3 => Ok(Geometry::Polygon(self.read_polygon(&header)?)),
            4 => Ok(Geometry::MultiPoint(MultiPoint(self.read_children(
                header.big_endian,
                |reader| match reader.read_geometry()? {
                    Geometry::Point(point) => Ok(point),
                    _ => Err("MultiPoint contained a non-point geometry".to_string()),
                },
            )?))),
            5 => Ok(Geometry::MultiLineString(MultiLineString(
                self.read_children(header.big_endian, |reader| {
                    match reader.read_geometry()? {
                        Geometry::LineString(line_string) => Ok(line_string),
                        _ => Err("MultiLineString contained a non-linestring geometry".to_string()),
                    }
                })?,
            ))),
            6 => Ok(Geometry::MultiPolygon(MultiPolygon(self.read_children(
                header.big_endian,
                |reader| match reader.read_geometry()? {
                    Geometry::Polygon(polygon) => Ok(polygon),
                    _ => Err("MultiPolygon contained a non-polygon geometry".to_string()),
                },
            )?))),
            7 => Err("GeometryCollection is not currently supported".to_string()),
            typ => Err(format!("unsupported geometry type {typ}")),
        }
    }

    fn read_header(&mut self) -> Result<EwkbHeader, String> {
        let big_endian = match self.read_u8()? {
            0 => true,
            1 => false,
            value => return Err(format!("invalid EWKB byte order {value}")),
        };
        let type_id = self.read_u32(big_endian)?;
        let has_z = type_id & Self::Z_FLAG != 0;
        let has_m = type_id & Self::M_FLAG != 0;
        if type_id & Self::SRID_FLAG != 0 {
            self.read_u32(big_endian)?;
        }

        let geometry_type = if has_z || has_m || type_id & Self::SRID_FLAG != 0 {
            type_id & !(Self::Z_FLAG | Self::M_FLAG | Self::SRID_FLAG)
        } else {
            // EWKB normally uses high-bit flags, but this also accepts ISO WKB Z/M/ZM type ids.
            match type_id / 1000 {
                1 => type_id - 1000,
                2 => type_id - 2000,
                3 => type_id - 3000,
                _ => type_id,
            }
        };
        let has_z = has_z || matches!(type_id / 1000, 1 | 3);
        let has_m = has_m || matches!(type_id / 1000, 2 | 3);

        Ok(EwkbHeader {
            big_endian,
            geometry_type,
            has_z,
            has_m,
        })
    }

    fn read_point(&mut self, header: &EwkbHeader) -> Result<Point<i32>, String> {
        let x = self.read_f64(header.big_endian)?;
        let y = self.read_f64(header.big_endian)?;
        if header.has_z {
            self.read_f64(header.big_endian)?;
        }
        if header.has_m {
            self.read_f64(header.big_endian)?;
        }
        Ok(Point::new(tile_coord(x), tile_coord(y)))
    }

    fn read_line_string(&mut self, header: &EwkbHeader) -> Result<LineString<i32>, String> {
        let len = self.read_len(header.big_endian)?;
        (0..len)
            .map(|_| {
                let point = self.read_point(header)?;
                Ok(Coord {
                    x: point.x(),
                    y: point.y(),
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(LineString)
    }

    fn read_polygon(&mut self, header: &EwkbHeader) -> Result<Polygon<i32>, String> {
        let len = self.read_len(header.big_endian)?;
        let mut rings = (0..len)
            .map(|_| self.read_line_string(header))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter();
        let exterior = rings.next().unwrap_or_else(|| LineString::new(vec![]));
        Ok(Polygon::new(exterior, rings.collect()))
    }

    fn read_children<T>(
        &mut self,
        parent_big_endian: bool,
        mut read_child: impl FnMut(&mut EwkbReader<'a>) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let len = self.read_len(parent_big_endian)?;
        (0..len).map(|_| read_child(self)).collect()
    }

    fn read_len(&mut self, big_endian: bool) -> Result<usize, String> {
        let len = self.read_u32(big_endian)?;
        usize::try_from(len).map_err(|_| "EWKB count does not fit in usize".to_string())
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        let value = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or_else(|| "unexpected end of EWKB".to_string())?;
        self.offset += 1;
        Ok(value)
    }

    fn read_u32(&mut self, big_endian: bool) -> Result<u32, String> {
        let bytes = self.read_array::<4>()?;
        Ok(if big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    }

    fn read_f64(&mut self, big_endian: bool) -> Result<f64, String> {
        let bytes = self.read_array::<8>()?;
        Ok(if big_endian {
            f64::from_be_bytes(bytes)
        } else {
            f64::from_le_bytes(bytes)
        })
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or_else(|| "EWKB offset overflow".to_string())?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "unexpected end of EWKB".to_string())?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| "failed to read EWKB bytes".to_string())
    }
}

fn tile_coord(value: f64) -> i32 {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        error!("PT_AsMLT geometry coordinate is outside the supported i32 tile range");
    }
    #[expect(clippy::cast_possible_truncation, reason = "range checked above")]
    {
        value.round() as i32
    }
}

fn quote_qualified_identifier(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_identifier(schema), quote_identifier(name))
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(any(test, feature = "pg_test"))]
#[pgrx::pg_schema]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::fmt::Write as _;

    use pgrx::prelude::*;

    use super::*;

    fn gzip_test(data: Option<&str>, level: Option<i32>) {
        let mut query = "SELECT pt_gzip(".to_string();
        if let Some(data) = data {
            write!(query, "'{data}'::bytea").unwrap();
        } else {
            write!(query, "NULL").unwrap();
        }
        if let Some(level) = level {
            write!(query, ", {level})").unwrap();
        } else {
            write!(query, ")").unwrap();
        }
        let result = Spi::get_one::<&[u8]>(&query).unwrap();
        let expected = data.map(|v| compression::pt_gzip(v.as_bytes(), level).unwrap());
        assert_eq!(result, expected.as_deref(), "{query}");
    }

    fn brotli_test(data: Option<&str>) {
        let mut query = "SELECT pt_brotli(".to_string();
        if let Some(data) = data {
            write!(query, "'{data}'::bytea)").unwrap();
        } else {
            write!(query, "NULL)").unwrap();
        }
        let result = Spi::get_one::<&[u8]>(&query).unwrap();
        let expected = data.map(|v| compression::pt_brotli(v.as_bytes()).unwrap());
        assert_eq!(result, expected.as_deref(), "{query}");
    }

    fn ewkb_header(geometry_type: u32) -> Vec<u8> {
        let mut bytes = vec![1];
        bytes.extend_from_slice(&geometry_type.to_le_bytes());
        bytes
    }

    fn ewkb_coord(bytes: &mut Vec<u8>, x: f64, y: f64) {
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
    }

    fn ewkb_point(x: f64, y: f64) -> Vec<u8> {
        let mut bytes = ewkb_header(1);
        ewkb_coord(&mut bytes, x, y);
        bytes
    }

    fn ewkb_point_with_srid(x: f64, y: f64) -> Vec<u8> {
        let mut bytes = ewkb_header(EwkbReader::SRID_FLAG | 1);
        bytes.extend_from_slice(&4326_u32.to_le_bytes());
        ewkb_coord(&mut bytes, x, y);
        bytes
    }

    fn ewkb_line_string(points: &[(f64, f64)]) -> Vec<u8> {
        let mut bytes = ewkb_header(2);
        bytes.extend_from_slice(&u32::try_from(points.len()).unwrap().to_le_bytes());
        for &(x, y) in points {
            ewkb_coord(&mut bytes, x, y);
        }
        bytes
    }

    fn ewkb_polygon(rings: &[&[(f64, f64)]]) -> Vec<u8> {
        let mut bytes = ewkb_header(3);
        bytes.extend_from_slice(&u32::try_from(rings.len()).unwrap().to_le_bytes());
        for ring in rings {
            bytes.extend_from_slice(&u32::try_from(ring.len()).unwrap().to_le_bytes());
            for &(x, y) in *ring {
                ewkb_coord(&mut bytes, x, y);
            }
        }
        bytes
    }

    fn ewkb_multi(geometry_type: u32, children: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = ewkb_header(geometry_type);
        bytes.extend_from_slice(&u32::try_from(children.len()).unwrap().to_le_bytes());
        for child in children {
            bytes.extend_from_slice(child);
        }
        bytes
    }

    fn assert_mlt_encodes(geometry: Geometry<i32>) {
        let mut layer = TileLayer::builder("test", 4096).unwrap();
        layer.feature(geometry).finish().unwrap();
        let tile = layer.finish().encode(EncoderConfig::default()).unwrap();
        assert!(!tile.is_empty());
    }

    #[pg_test]
    fn test_ewkb_basic_geometry_families() {
        let point = parse_ewkb_geometry(&ewkb_point(10.0, 20.0));
        assert!(matches!(point, Geometry::Point(_)));
        assert_mlt_encodes(point);

        let line = parse_ewkb_geometry(&ewkb_line_string(&[(0.0, 0.0), (10.0, 10.0)]));
        assert!(matches!(line, Geometry::LineString(_)));
        assert_mlt_encodes(line);

        let polygon = parse_ewkb_geometry(&ewkb_polygon(&[&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 0.0),
        ]]));
        assert!(matches!(polygon, Geometry::Polygon(_)));
        assert_mlt_encodes(polygon);

        let multi_point = parse_ewkb_geometry(&ewkb_multi(
            4,
            &[ewkb_point(1.0, 2.0), ewkb_point(3.0, 4.0)],
        ));
        assert!(matches!(multi_point, Geometry::MultiPoint(_)));
        assert_mlt_encodes(multi_point);

        let multi_line = parse_ewkb_geometry(&ewkb_multi(
            5,
            &[
                ewkb_line_string(&[(0.0, 0.0), (1.0, 1.0)]),
                ewkb_line_string(&[(2.0, 2.0), (3.0, 3.0)]),
            ],
        ));
        assert!(matches!(multi_line, Geometry::MultiLineString(_)));
        assert_mlt_encodes(multi_line);

        let multi_polygon = parse_ewkb_geometry(&ewkb_multi(
            6,
            &[
                ewkb_polygon(&[&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]]),
                ewkb_polygon(&[&[(2.0, 2.0), (3.0, 2.0), (3.0, 3.0), (2.0, 2.0)]]),
            ],
        ));
        assert!(matches!(multi_polygon, Geometry::MultiPolygon(_)));
        assert_mlt_encodes(multi_polygon);
    }

    #[pg_test]
    fn test_ewkb_flags_preserve_full_geometry_type() {
        let point = parse_ewkb_geometry(&ewkb_point_with_srid(10.0, 20.0));
        assert!(matches!(point, Geometry::Point(_)));

        let unsupported_type = 300;
        let mut bytes = ewkb_header(EwkbReader::SRID_FLAG | unsupported_type);
        bytes.extend_from_slice(&4326_u32.to_le_bytes());
        let err = EwkbReader::new(&bytes).read_geometry().unwrap_err();
        assert_eq!(err, "unsupported geometry type 300");
    }

    #[pg_test]
    fn test_pt_gzip() {
        gzip_test(None, None);
        gzip_test(None, Some(5));

        let data = Some("");
        gzip_test(data, None);

        let data = Some("Hello");
        gzip_test(data, Some(0));
        gzip_test(data, Some(1));
        gzip_test(data, Some(9));
        gzip_test(data, None);
    }

    #[pg_test]
    fn test_pt_brotli() {
        brotli_test(None);
        brotli_test(Some(""));
        brotli_test(Some("Hello"));
    }

    #[pg_test]
    fn test_pt_asmlt() {
        Spi::run(
            "
CREATE TEMP TABLE pt_asmlt_test (
    id bigint,
    geom point,
    name text,
    rank integer
)",
        )
        .unwrap();
        Spi::run(
            "
INSERT INTO pt_asmlt_test (id, geom, name, rank)
VALUES (1, point(10, 20), 'one', 7), (2, point(30, 40), 'two', 9)",
        )
        .unwrap();

        let tile = Spi::get_one::<&[u8]>(
            "SELECT PT_AsMLT('pt_asmlt_test'::regclass, 'test_layer', 4096, 'geom', 'id')",
        )
        .unwrap()
        .unwrap();

        assert!(!tile.is_empty());
    }

    #[pg_test(error = "PT_AsMLT feature_id_name \"missing_id\" does not exist")]
    fn test_pt_asmlt_rejects_unknown_feature_id() {
        Spi::run(
            "
CREATE TEMP TABLE pt_asmlt_missing_feature_id_test (
    id bigint,
    geom point
)",
        )
        .unwrap();
        Spi::run(
            "
INSERT INTO pt_asmlt_missing_feature_id_test (id, geom)
VALUES (1, point(10, 20))",
        )
        .unwrap();

        Spi::get_one::<&[u8]>(
            "SELECT PT_AsMLT('pt_asmlt_missing_feature_id_test'::regclass, 'test_layer', 4096, 'geom', 'missing_id')",
        )
        .unwrap();
    }
}
