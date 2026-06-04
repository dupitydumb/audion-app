use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub admin_user: String,
    pub admin_password_raw: String,
    pub jwt_secret: String,
    pub data_dir: PathBuf,
    pub port: u16,
}

impl Config {
    pub fn load() -> Self {
        let admin_user = std::env::var("AUDION_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let admin_password_raw = std::env::var("AUDION_ADMIN_PASSWORD").unwrap_or_else(|_| "changeme".to_string());
        let jwt_secret = std::env::var("AUDION_JWT_SECRET").unwrap_or_else(|_| "your-secret-key-here-super-secure".to_string());
        
        let data_dir_str = std::env::var("AUDION_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let data_dir = PathBuf::from(data_dir_str);

        let port = std::env::var("AUDION_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        Config {
            admin_user,
            admin_password_raw,
            jwt_secret,
            data_dir,
            port,
        }
    }

    pub fn music_dir(&self) -> PathBuf {
        self.data_dir.join("music")
    }

    pub fn artwork_dir(&self) -> PathBuf {
        self.data_dir.join("artwork")
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("db").join("audion.sqlite")
    }
}
