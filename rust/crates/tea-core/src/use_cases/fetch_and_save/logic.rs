use super::domain::{Cmd, Model, Msg, UpdateResult};

pub fn init() -> Model {
    Model { saved: false }
}

pub fn update(model: Model, msg: Msg) -> UpdateResult {
    let mut next = model;
    let mut command = Cmd::None;

    match msg {
        Msg::HttpResponseReady { success } => {
            next.saved = success;
            if success {
                command = Cmd::SaveIso8601File;
            }
        }
    }

    UpdateResult { next, command }
}
