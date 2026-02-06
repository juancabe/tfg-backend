pub mod auth {
    use serde::{Deserialize, Serialize};
    #[derive(Serialize, Deserialize, Debug)]
    pub struct EmailPass {
        pub email: String,
        pub hashed_pass: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum AuthMethod {
        EmailPass(EmailPass),
    }
}
