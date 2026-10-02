use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { synced: false }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::DbQueryResultReady { success } => {
            if success {
                command = Cmd::SendUart;
            }
        }
        Msg::UartTxDone => {
            command = Cmd::SyncNetwork;
            next.synced = true;
        }
    }

    UpdateResult { next, command }
}
