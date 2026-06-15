use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::now_v7().simple().to_string()
}
