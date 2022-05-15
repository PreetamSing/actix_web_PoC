use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EnvConfig {
  pub db_string: String,
  pub port: u16,
  pub shell_env_var: String,
}
