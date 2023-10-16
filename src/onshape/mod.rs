pub mod client;
pub mod models;

use anyhow::Result;
use dotenv::dotenv;
use url::Url;

use self::client::OnShapeClient;

pub(crate) fn environment_client(proxy_url: Option<Url>) -> Result<OnShapeClient> {
    dotenv().ok();

    OnShapeClient::new(
        std::env::var("ONSHAPE_ACCESS_KEY")?,
        std::env::var("ONSHAPE_SECRET_KEY")?,
        proxy_url,
    )
}
