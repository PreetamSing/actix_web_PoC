# actix_web_PoC
Informal PoC to depict integration of `actix_web` with `config`, `reqwest`, `mongodb`.

# Setup
- Clone the repo.
- Create a `Settings.toml` file based on directions in `Settings.sample.toml`.
- Export `APP_SHELL_ENV_VAR` set to random string value in the terminal ( value doesn't matter as it isn't used anywhere ).
  #### Linux:
  ```bash
  export APP_SHELL_ENV_VAR=abcd
  ```
  #### Windows Powershell:
  ```powershell
  SET APP_SHELL_ENV_VAR=abcd
  ```
- Run following command in terminal after moving into project directory:
  ```bash
  cargo run
  ```
