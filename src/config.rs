use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub admin_user: String,
    pub admin_password_raw: String,
    pub jwt_secret: String,
    pub data_dir: PathBuf,
    pub port: u16,
    pub public_dir: PathBuf,
}

impl Config {
    pub fn load() -> Self {
        let admin_user = std::env::var("AUDION_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        
        let admin_password_raw = std::env::var("AUDION_ADMIN_PASSWORD").unwrap_or_else(|_| {
            eprintln!("[SECURITY WARNING] AUDION_ADMIN_PASSWORD is not set. Using default password 'changeme'. Please change it!");
            "changeme".to_string()
        });
        
        let jwt_secret = std::env::var("AUDION_JWT_SECRET").unwrap_or_else(|_| {
            eprintln!("[SECURITY WARNING] AUDION_JWT_SECRET is not set. Using insecure default secret key. DO NOT DO THIS IN PRODUCTION!");
            "your-secret-key-here-super-secure".to_string()
        });

        if jwt_secret == "your-secret-key-here-change-this-in-production" {
            eprintln!("[SECURITY WARNING] AUDION_JWT_SECRET is set to the default docker-compose secret. Please configure a unique secret in production!");
        }

        
        let data_dir_str = std::env::var("AUDION_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let data_dir = PathBuf::from(data_dir_str);

        let port = std::env::var("AUDION_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        let public_dir_str = std::env::var("AUDION_PUBLIC_DIR").unwrap_or_else(|_| "./frontend/dist".to_string());
        let public_dir = PathBuf::from(public_dir_str);

        Config {
            admin_user,
            admin_password_raw,
            jwt_secret,
            data_dir,
            port,
            public_dir,
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

    pub fn user_dir(&self, user_id: &str) -> PathBuf {
        self.data_dir.join("users").join(user_id)
    }

    pub fn user_music_dir(&self, user_id: &str) -> PathBuf {
        self.user_dir(user_id).join("music")
    }

    pub fn user_artwork_dir(&self, user_id: &str) -> PathBuf {
        self.user_dir(user_id).join("artwork")
    }

    pub fn user_db_path(&self, user_id: &str) -> PathBuf {
        self.user_dir(user_id).join("db").join("audion.sqlite")
    }
}
