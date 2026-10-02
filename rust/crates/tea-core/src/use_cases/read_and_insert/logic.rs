use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { rows_logged: 0 }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;

    let command = match msg {
        Msg::AdcRowReady { millivolts } => {
            next.rows_logged = model.rows_logged + 1;
            Cmd::InsertDbRow { millivolts }
        }
    };

    UpdateResult { next, command }
}
