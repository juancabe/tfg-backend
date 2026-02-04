use crate::dao::{DaoError, DbPool};
use crate::model::models::{Command, NewCommand};
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
    fn queue_command(&self, cmd: NewCommand) -> Result<Command, DaoError> {
        todo!()
    }

    fn fetch_pending_commands(&self, producer_id: Uuid) -> Result<Vec<Command>, DaoError> {
        todo!()
    }

    fn get_command_history(&self, producer_id: Uuid) -> Result<Vec<Command>, DaoError> {
        todo!()
    }
}
