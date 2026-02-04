use crate::dao::{DaoError, DbPool};
use crate::model::models::{Command, NewCommand};
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

pub trait CommandDao {
    fn queue_command(&self, cmd: NewCommand) -> Result<Command, DaoError>;
    fn fetch_pending_commands(&self, producer_id: Uuid) -> Result<Vec<Command>, DaoError>;
    fn get_command_history(&self, producer_id: Uuid) -> Result<Vec<Command>, DaoError>;
}

pub struct PgCommandDao {
    pool: DbPool,
}

impl PgCommandDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl CommandDao for PgCommandDao {
    fn queue_command(&self, new_cmd: NewCommand) -> Result<Command, DaoError> {
        use crate::schema::commands::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(commands)
            .values(&new_cmd)
            .returning(Command::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn fetch_pending_commands(&self, producer_id_in: Uuid) -> Result<Vec<Command>, DaoError> {
        use crate::schema::commands::dsl::*;
        let conn = &mut self.pool.get()?;

        // CommandStatus::Sent = 1 (pending)
        commands
            .filter(target_producer_id.eq(producer_id_in))
            .filter(status.eq(1))
            .order(created_at.asc())
            .select(Command::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    fn get_command_history(&self, producer_id_in: Uuid) -> Result<Vec<Command>, DaoError> {
        use crate::schema::commands::dsl::*;
        let conn = &mut self.pool.get()?;
        commands
            .filter(target_producer_id.eq(producer_id_in))
            .order(created_at.desc())
            .select(Command::as_select())
            .load(conn)
            .map_err(Into::into)
    }
}
