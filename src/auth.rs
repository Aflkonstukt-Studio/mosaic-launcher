use anyhow::{Result, anyhow};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl,
    devicecode::{DeviceAuthorizationResponse, EmptyExtraDeviceAuthorizationFields},
    StandardTokenResponse, EmptyExtraTokenFields, TokenResponse,
};
use reqwest::blocking::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use log::{info, error, debug, warn};
use uuid::Uuid;

const MS_CLIENT_ID: &str = "2533faf1-dfcc-4f08-9520-83d421e91426";
const MS_AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const MS_DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode";
const MC_XBOX_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const MC_XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const MC_OWNERSHIP_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skins: Vec<MinecraftSkin>,
    pub capes: Vec<MinecraftCape>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftCape {
    pub id: String,
    pub state: String,
    pub url: String,
    pub alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub minecraft_token: Option<String>,
    pub minecraft_profile: Option<MinecraftProfile>,
    pub is_offline: bool,
}

pub struct AuthManager {
    http_client: HttpClient,
    oauth_client: BasicClient,
}

impl AuthManager {
    pub fn new() -> Self {
        let oauth_client = BasicClient::new(
            ClientId::new(MS_CLIENT_ID.to_string()),
            None,
            AuthUrl::new(MS_AUTH_URL.to_string()).unwrap(),
            Some(TokenUrl::new(MS_TOKEN_URL.to_string()).unwrap()),
        )
            .set_device_authorization_url(DeviceAuthorizationUrl::new(MS_DEVICE_CODE_URL.to_string()).unwrap());

        Self {
            http_client: HttpClient::new(),
            oauth_client,
        }
    }

    pub fn start_login(&self) -> Result<DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields>> {
        let scopes = vec![
            "XboxLive.signin".to_string(),
            "XboxLive.offline_access".to_string(),
        ];

        let device_auth = self.oauth_client
            .exchange_device_code()
            .map_err(|e| anyhow!("Failed to create device code request: {}", e))?
            .add_scopes(scopes.iter().map(|s| Scope::new(s.clone())))
            .request(oauth2::reqwest::http_client)?;

        Ok(device_auth)
    }

    pub fn poll_for_token(
        &self,
        device_auth: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields>,
    ) -> Result<CustomTokenResponse> {
        let start_time = Instant::now();
        let expires_in = device_auth.expires_in();

        loop {
            if start_time.elapsed() >= expires_in {
                return Err(anyhow!("Device code expired"));
            }

            match self.oauth_client
                .exchange_device_access_token(&device_auth)
                .request(
                    oauth2::reqwest::http_client,
                    std::thread::sleep,
                    None
                )
            {
                Ok(token) => {
                    let access_token = token.access_token().secret().to_string();
                    let refresh_token = token.refresh_token().map(|rt| rt.secret().to_string());
                    let expires_at = token.expires_in().map(|d| {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        now + d.as_secs()
                    });
                    return Ok(CustomTokenResponse {
                        access_token,
                        refresh_token,
                        expires_at,
                    });
                }
                Err(oauth2::RequestTokenError::ServerResponse(err))
                if *err.error() == oauth2::devicecode::DeviceCodeErrorResponseType::AuthorizationPending =>
                    {
                        std::thread::sleep(device_auth.interval());
                    }
                Err(e) => return Err(anyhow!("Failed to get access token: {}", e)),
            }
        }
    }

    pub fn authenticate_with_xbox(&self, ms_token: &str) -> Result<String> {
        #[derive(Serialize)]
        struct XboxAuthRequest {
            Properties: XboxAuthProperties,
            RelyingParty: String,
            TokenType: String,
        }
        #[derive(Serialize)]
        struct XboxAuthProperties {
            AuthMethod: String,
            SiteName: String,
            RpsTicket: String,
        }
        #[derive(Deserialize)]
        struct XboxAuthResponse {
            Token: String,
        }

        let request = XboxAuthRequest {
            Properties: XboxAuthProperties {
                AuthMethod: "RPS".to_string(),
                SiteName: "user.auth.xboxlive.com".to_string(),
                RpsTicket: format!("d={}", ms_token),
            },
            RelyingParty: "http://auth.xboxlive.com".to_string(),
            TokenType: "JWT".to_string(),
        };

        let response = self.http_client
            .post(MC_XBOX_AUTH_URL)
            .json(&request)
            .send()?
            .error_for_status()?;

        let xbox_response: XboxAuthResponse = response.json()?;
        Ok(xbox_response.Token)
    }

    pub fn get_xsts_token(&self, xbox_token: &str) -> Result<(String, String)> {
        #[derive(Serialize)]
        struct XstsAuthRequest {
            Properties: XstsAuthProperties,
            RelyingParty: String,
            TokenType: String,
        }
        #[derive(Serialize)]
        struct XstsAuthProperties {
            SandboxId: String,
            UserTokens: Vec<String>,
        }
        #[derive(Deserialize)]
        struct XstsAuthResponse {
            Token: String,
            #[serde(rename = "DisplayClaims")]
            display_claims: XstsDisplayClaims,
        }
        #[derive(Deserialize)]
        struct XstsDisplayClaims {
            xui: Vec<XstsXui>,
        }
        #[derive(Deserialize)]
        struct XstsXui {
            uhs: String,
        }

        let request = XstsAuthRequest {
            Properties: XstsAuthProperties {
                SandboxId: "RETAIL".to_string(),
                UserTokens: vec![xbox_token.to_string()],
            },
            RelyingParty: "rp://api.minecraftservices.com/".to_string(),
            TokenType: "JWT".to_string(),
        };

        let response = self.http_client
            .post(MC_XSTS_AUTH_URL)
            .json(&request)
            .send()?
            .error_for_status()?;

        let xsts_response: XstsAuthResponse = response.json()?;
        let uhs = xsts_response.display_claims.xui.get(0)
            .ok_or_else(|| anyhow!("No Xbox user hash found in XSTS response"))?
            .uhs.clone();

        Ok((xsts_response.Token, uhs))
    }

    pub fn authenticate_with_minecraft(&self, xsts_data: &(String, String)) -> Result<String> {
        #[derive(Serialize)]
        struct MinecraftAuthRequest {
            identityToken: String,
        }
        #[derive(Deserialize)]
        struct MinecraftAuthResponse {
            access_token: String,
        }

        let (xsts_token, xsts_uhs) = xsts_data;
        let identity_token = format!("XBL3.0 x={};{}", xsts_uhs, xsts_token);

        let request = MinecraftAuthRequest {
            identityToken: identity_token,
        };

        let response = self.http_client
            .post(MC_LOGIN_URL)
            .json(&request)
            .send()?
            .error_for_status()?;

        let minecraft_response: MinecraftAuthResponse = response.json()?;
        Ok(minecraft_response.access_token)
    }

    pub fn check_game_ownership(&self, minecraft_token: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct OwnershipResponse {
            items: Vec<OwnershipItem>,
        }
        #[derive(Deserialize)]
        struct OwnershipItem {
            name: String,
        }

        let response = self.http_client
            .get(MC_OWNERSHIP_URL)
            .header("Authorization", format!("Bearer {}", minecraft_token))
            .send()?
            .error_for_status()?;

        let ownership_response: OwnershipResponse = response.json()?;
        if ownership_response.items.iter().any(|item| item.name == "product_minecraft") {
            Ok(())
        } else {
            Err(anyhow!("This Microsoft account does not own Minecraft."))
        }
    }

    pub fn get_minecraft_profile(&self, minecraft_token: &str) -> Result<MinecraftProfile> {
        let response = self.http_client
            .get(MC_PROFILE_URL)
            .header("Authorization", format!("Bearer {}", minecraft_token))
            .send()?
            .error_for_status()?;

        let profile: MinecraftProfile = response.json()?;
        Ok(profile)
    }

    pub async fn complete_login(&self, device_auth: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields>) -> Result<AuthSession> {
        // Step 1: Poll for the token
        let token_response = self.poll_for_token(device_auth)?;

        // Step 2: Authenticate with Xbox
        let xbox_token = self.authenticate_with_xbox(&token_response.access_token)?;

        // Step 3: Get XSTS token
        let xsts_data = self.get_xsts_token(&xbox_token)?;

        // Step 4: Authenticate with Minecraft
        let minecraft_token = self.authenticate_with_minecraft(&xsts_data)?;

        // Step 5: Check game ownership
        self.check_game_ownership(&minecraft_token)?;

        // Step 6: Get Minecraft profile
        let minecraft_profile = self.get_minecraft_profile(&minecraft_token)?;

        // Create and return the auth session
        Ok(AuthSession {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at: token_response.expires_at,
            minecraft_token: Some(minecraft_token),
            minecraft_profile: Some(minecraft_profile),
            is_offline: false,
        })
    }

    // Create an offline session with a custom username
    pub fn create_offline_session(&self, username: &str) -> Result<AuthSession> {
        info!("Creating offline session for username: {}", username);

        if username.is_empty() {
            return Err(anyhow!("Username cannot be empty"));
        }

        // Generate a UUID for the offline player
        // In a real implementation, we would use a deterministic UUID based on the username
        // but for simplicity, we'll use a random UUID
        let uuid = Uuid::new_v4();

        // Create a minimal Minecraft profile
        let minecraft_profile = MinecraftProfile {
            id: uuid.to_string().replace("-", ""), // Minecraft uses UUIDs without hyphens
            name: username.to_string(),
            skins: vec![],
            capes: vec![],
        };

        // Create and return the offline auth session
        Ok(AuthSession {
            access_token: "offline".to_string(),
            refresh_token: None,
            expires_at: None,
            minecraft_token: Some("offline".to_string()),
            minecraft_profile: Some(minecraft_profile),
            is_offline: true,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CustomTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
}
