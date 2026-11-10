//! Esquema leído del servidor: tablas, columnas, claves primarias, índices y relaciones (claves foráneas).
//!
//! Cada motor tiene tres consultas que devuelven **una fila por columna** con el mismo orden de campos; aquí se
//! agrupan. Así no hay `string_agg` ni `GROUP_CONCAT` (que cambian de versión en versión) y un nombre con comas
//! no rompe nada. Todos los nombres son del servidor (entrada no confiable): escápalos con `sanitize::display`
//! al mostrarlos; para generar SQL usa los valores tal cual y entrecomíllalos.

use super::driver::ResultSet;
use super::store::Engine;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TableRef {
    pub(crate) schema: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Column {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) nullable: bool,
    pub(crate) default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Index {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) unique: bool,
    pub(crate) primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Table {
    pub(crate) reference: TableRef,
    pub(crate) is_view: bool,
    pub(crate) columns: Vec<Column>,
    pub(crate) indexes: Vec<Index>,
}

impl Table {
    pub(crate) fn primary_key(&self) -> &[String] {
        self.indexes
            .iter()
            .find(|i| i.primary)
            .map(|i| i.columns.as_slice())
            .unwrap_or(&[])
    }
}

/// Clave foránea: `from` (con `from_columns`) apunta a `to` (con `to_columns`), columna a columna
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Relation {
    pub(crate) name: String,
    pub(crate) from: TableRef,
    pub(crate) from_columns: Vec<String>,
    pub(crate) to: TableRef,
    pub(crate) to_columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Schema {
    pub(crate) tables: Vec<Table>,
    pub(crate) relations: Vec<Relation>,
}

impl Schema {
    pub(crate) fn table(&self, reference: &TableRef) -> Option<&Table> {
        self.tables.iter().find(|t| &t.reference == reference)
    }
}

/// Las tres consultas de un motor. Campos de cada una:
/// * `columns`: esquema, tabla, tipo de tabla, columna, tipo de dato, anulable (`YES`/`NO`), valor por defecto
/// * `indexes`: esquema, tabla, índice, único (0/1), primario (0/1), columna (en orden)
/// * `foreign_keys`: restricción, esquema, tabla, columna, esquema referenciado, tabla referenciada, columna referenciada
pub(crate) struct SchemaQueries {
    pub(crate) columns: &'static str,
    pub(crate) indexes: &'static str,
    pub(crate) foreign_keys: &'static str,
}

const PG: SchemaQueries = SchemaQueries {
    columns: "SELECT c.table_schema, c.table_name, t.table_type, c.column_name, c.data_type, c.is_nullable, c.column_default \
              FROM information_schema.columns c \
              JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name \
              WHERE c.table_schema NOT IN ('pg_catalog', 'information_schema') \
              ORDER BY c.table_schema, c.table_name, c.ordinal_position",
    indexes: "SELECT ns.nspname, t.relname, i.relname, ix.indisunique::int, ix.indisprimary::int, a.attname \
              FROM pg_index ix \
              JOIN pg_class i ON i.oid = ix.indexrelid \
              JOIN pg_class t ON t.oid = ix.indrelid \
              JOIN pg_namespace ns ON ns.oid = t.relnamespace \
              CROSS JOIN LATERAL unnest(ix.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
              JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum \
              WHERE ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast') \
              ORDER BY ns.nspname, t.relname, i.relname, k.ord",
    foreign_keys: "SELECT con.conname, ns.nspname, cl.relname, a.attname, rns.nspname, rcl.relname, ra.attname \
                   FROM pg_constraint con \
                   JOIN pg_class cl ON cl.oid = con.conrelid \
                   JOIN pg_namespace ns ON ns.oid = cl.relnamespace \
                   JOIN pg_class rcl ON rcl.oid = con.confrelid \
                   JOIN pg_namespace rns ON rns.oid = rcl.relnamespace \
                   CROSS JOIN LATERAL unnest(con.conkey, con.confkey) WITH ORDINALITY AS k(attnum, rattnum, ord) \
                   JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum \
                   JOIN pg_attribute ra ON ra.attrelid = con.confrelid AND ra.attnum = k.rattnum \
                   WHERE con.contype = 'f' AND ns.nspname NOT IN ('pg_catalog', 'information_schema') \
                   ORDER BY ns.nspname, cl.relname, con.conname, k.ord",
};

const MYSQL: SchemaQueries = SchemaQueries {
    columns: "SELECT c.table_schema, c.table_name, t.table_type, c.column_name, c.column_type, c.is_nullable, c.column_default \
              FROM information_schema.columns c \
              JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name \
              WHERE c.table_schema = DATABASE() \
              ORDER BY c.table_name, c.ordinal_position",
    indexes: "SELECT s.table_schema, s.table_name, s.index_name, CASE WHEN s.non_unique = 0 THEN 1 ELSE 0 END, \
              CASE WHEN s.index_name = 'PRIMARY' THEN 1 ELSE 0 END, s.column_name \
              FROM information_schema.statistics s \
              WHERE s.table_schema = DATABASE() AND s.column_name IS NOT NULL \
              ORDER BY s.table_name, s.index_name, s.seq_in_index",
    foreign_keys: "SELECT k.constraint_name, k.table_schema, k.table_name, k.column_name, \
                   k.referenced_table_schema, k.referenced_table_name, k.referenced_column_name \
                   FROM information_schema.key_column_usage k \
                   WHERE k.table_schema = DATABASE() AND k.referenced_table_name IS NOT NULL \
                   ORDER BY k.table_name, k.constraint_name, k.ordinal_position",
};

const MSSQL: SchemaQueries = SchemaQueries {
    columns: "SELECT c.TABLE_SCHEMA, c.TABLE_NAME, t.TABLE_TYPE, c.COLUMN_NAME, c.DATA_TYPE, c.IS_NULLABLE, c.COLUMN_DEFAULT \
              FROM INFORMATION_SCHEMA.COLUMNS c \
              JOIN INFORMATION_SCHEMA.TABLES t ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME \
              ORDER BY c.TABLE_SCHEMA, c.TABLE_NAME, c.ORDINAL_POSITION",
    indexes: "SELECT s.name, t.name, i.name, CAST(i.is_unique AS int), CAST(i.is_primary_key AS int), c.name \
              FROM sys.indexes i \
              JOIN sys.tables t ON t.object_id = i.object_id \
              JOIN sys.schemas s ON s.schema_id = t.schema_id \
              JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.is_included_column = 0 \
              JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
              WHERE i.name IS NOT NULL \
              ORDER BY s.name, t.name, i.name, ic.key_ordinal",
    foreign_keys: "SELECT fk.name, ps.name, pt.name, pc.name, rs.name, rt.name, rc.name \
                   FROM sys.foreign_keys fk \
                   JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id \
                   JOIN sys.tables pt ON pt.object_id = fkc.parent_object_id \
                   JOIN sys.schemas ps ON ps.schema_id = pt.schema_id \
                   JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id \
                   JOIN sys.tables rt ON rt.object_id = fkc.referenced_object_id \
                   JOIN sys.schemas rs ON rs.schema_id = rt.schema_id \
                   JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id \
                   ORDER BY ps.name, pt.name, fk.name, fkc.constraint_column_id",
};

pub(crate) fn queries(engine: Engine) -> &'static SchemaQueries {
    match engine {
        Engine::Postgres => &PG,
        Engine::Mysql | Engine::Mariadb => &MYSQL,
        Engine::SqlServer => &MSSQL,
    }
}

fn text(row: &[super::driver::Cell], index: usize) -> Option<&str> {
    row.get(index).and_then(|cell| cell.raw())
}

fn owned(row: &[super::driver::Cell], index: usize) -> String {
    text(row, index).unwrap_or_default().to_string()
}

fn truthy(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "t" | "true" | "yes"))
}

fn table_ref(row: &[super::driver::Cell], schema_at: usize) -> TableRef {
    TableRef {
        schema: owned(row, schema_at),
        name: owned(row, schema_at + 1),
    }
}

/// Une los tres resultados en un `Schema`. Las filas con campos que faltan se ignoran.
pub(crate) fn build(columns: &ResultSet, indexes: &ResultSet, foreign_keys: &ResultSet) -> Schema {
    let mut schema = Schema::default();
    let mut position: HashMap<TableRef, usize> = HashMap::new();

    for row in &columns.rows {
        let reference = table_ref(row, 0);
        let slot = *position.entry(reference.clone()).or_insert_with(|| {
            schema.tables.push(Table {
                reference,
                is_view: text(row, 2).is_some_and(|t| t.to_ascii_uppercase().contains("VIEW")),
                columns: Vec::new(),
                indexes: Vec::new(),
            });
            schema.tables.len() - 1
        });
        schema.tables[slot].columns.push(Column {
            name: owned(row, 3),
            data_type: owned(row, 4),
            nullable: text(row, 5).is_some_and(|v| v.eq_ignore_ascii_case("YES")),
            default: text(row, 6).map(str::to_string),
        });
    }

    for row in &indexes.rows {
        let Some(&slot) = position.get(&table_ref(row, 0)) else {
            continue;
        };
        let name = owned(row, 2);
        let column = owned(row, 5);
        let table = &mut schema.tables[slot];
        match table.indexes.iter_mut().find(|i| i.name == name) {
            Some(index) => index.columns.push(column),
            None => table.indexes.push(Index {
                name,
                columns: vec![column],
                unique: truthy(text(row, 3)),
                primary: truthy(text(row, 4)),
            }),
        }
    }

    for row in &foreign_keys.rows {
        let name = owned(row, 0);
        let from = table_ref(row, 1);
        let existing = schema
            .relations
            .iter_mut()
            .find(|r| r.name == name && r.from == from);
        match existing {
            Some(relation) => {
                relation.from_columns.push(owned(row, 3));
                relation.to_columns.push(owned(row, 6));
            }
            None => schema.relations.push(Relation {
                name,
                from,
                from_columns: vec![owned(row, 3)],
                to: table_ref(row, 4),
                to_columns: vec![owned(row, 6)],
            }),
        }
    }
    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::driver::Cell;

    fn set(rows: &[&[Option<&str>]]) -> ResultSet {
        ResultSet {
            rows: rows
                .iter()
                .map(|r| r.iter().map(|v| Cell::new(v.map(str::to_string))).collect())
                .collect(),
            ..ResultSet::default()
        }
    }

    #[test]
    fn rows_per_column_are_grouped_into_tables_indexes_and_relations() {
        let columns = set(&[
            &[
                Some("public"),
                Some("users"),
                Some("BASE TABLE"),
                Some("id"),
                Some("integer"),
                Some("NO"),
                Some("nextval('x')"),
            ],
            &[
                Some("public"),
                Some("users"),
                Some("BASE TABLE"),
                Some("email"),
                Some("text"),
                Some("YES"),
                None,
            ],
            &[
                Some("public"),
                Some("orders"),
                Some("BASE TABLE"),
                Some("id"),
                Some("integer"),
                Some("NO"),
                None,
            ],
            &[
                Some("public"),
                Some("orders"),
                Some("BASE TABLE"),
                Some("user_id"),
                Some("integer"),
                Some("NO"),
                None,
            ],
            &[
                Some("public"),
                Some("v_users"),
                Some("VIEW"),
                Some("id"),
                Some("integer"),
                Some("YES"),
                None,
            ],
        ]);
        let indexes = set(&[
            &[
                Some("public"),
                Some("users"),
                Some("users_pkey"),
                Some("1"),
                Some("1"),
                Some("id"),
            ],
            &[
                Some("public"),
                Some("users"),
                Some("users_email_key"),
                Some("t"),
                Some("f"),
                Some("email"),
            ],
            &[
                Some("public"),
                Some("orders"),
                Some("orders_pkey"),
                Some("1"),
                Some("1"),
                Some("id"),
            ],
            &[
                Some("public"),
                Some("orders"),
                Some("by_user_date"),
                Some("0"),
                Some("0"),
                Some("user_id"),
            ],
            &[
                Some("public"),
                Some("orders"),
                Some("by_user_date"),
                Some("0"),
                Some("0"),
                Some("id"),
            ],
            &[
                Some("public"),
                Some("desconocida"),
                Some("x"),
                Some("0"),
                Some("0"),
                Some("c"),
            ],
        ]);
        let fks = set(&[&[
            Some("orders_user_fk"),
            Some("public"),
            Some("orders"),
            Some("user_id"),
            Some("public"),
            Some("users"),
            Some("id"),
        ]]);
        let schema = build(&columns, &indexes, &fks);

        assert_eq!(schema.tables.len(), 3);
        let users = &schema.tables[0];
        assert_eq!(
            users.reference,
            TableRef {
                schema: "public".into(),
                name: "users".into()
            }
        );
        assert_eq!(users.columns.len(), 2);
        assert!(users.columns[0].default.is_some() && !users.columns[0].nullable);
        assert!(users.columns[1].nullable && users.columns[1].default.is_none());
        assert_eq!(users.primary_key(), ["id"]);
        assert!(users.indexes[1].unique && !users.indexes[1].primary);
        assert!(schema.tables[2].is_view && !users.is_view);

        let orders = &schema.tables[1];
        assert_eq!(orders.indexes[1].columns, ["user_id", "id"]);
        assert_eq!(schema.relations.len(), 1);
        let relation = &schema.relations[0];
        assert_eq!(
            (relation.from.name.as_str(), relation.to.name.as_str()),
            ("orders", "users")
        );
        assert_eq!(
            (
                relation.from_columns.as_slice(),
                relation.to_columns.as_slice()
            ),
            (&["user_id".to_string()][..], &["id".to_string()][..])
        );
    }

    #[test]
    fn composite_foreign_keys_keep_column_pairs_in_order() {
        let fks = set(&[
            &[
                Some("fk"),
                Some("s"),
                Some("a"),
                Some("x1"),
                Some("s"),
                Some("b"),
                Some("y1"),
            ],
            &[
                Some("fk"),
                Some("s"),
                Some("a"),
                Some("x2"),
                Some("s"),
                Some("b"),
                Some("y2"),
            ],
            &[
                Some("fk2"),
                Some("s"),
                Some("a"),
                Some("z"),
                Some("s"),
                Some("c"),
                Some("id"),
            ],
        ]);
        let schema = build(&ResultSet::default(), &ResultSet::default(), &fks);
        assert_eq!(schema.relations.len(), 2);
        assert_eq!(schema.relations[0].from_columns, ["x1", "x2"]);
        assert_eq!(schema.relations[0].to_columns, ["y1", "y2"]);
    }

    #[test]
    fn every_engine_has_three_queries() {
        for engine in [
            Engine::Postgres,
            Engine::Mysql,
            Engine::Mariadb,
            Engine::SqlServer,
        ] {
            let q = queries(engine);
            assert!([q.columns, q.indexes, q.foreign_keys]
                .iter()
                .all(|s| s.starts_with("SELECT")));
        }
    }
}
