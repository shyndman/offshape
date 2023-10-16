use std::{collections::HashMap, str::FromStr, time::SystemTime};

use anyhow::{anyhow, Result};
use base64::Engine as _;
use bytes::Bytes;
use camino::Utf8PathBuf;
use governor::{
    clock::{Clock, QuantaClock},
    DefaultDirectRateLimiter, Quota, RateLimiter,
};
use hmac::{Hmac, Mac};
use http::header;
use lazy_static::lazy_static;
use nonzero_ext::nonzero;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use regex::Regex;
use reqwest::{
    blocking::{ClientBuilder, RequestBuilder, Response},
    redirect::Policy,
    IntoUrl, Method, Proxy, Url,
};
use sha2::Sha256;

use super::models::{
    DocumentElement, ExportFileFormat, Part, TranslationJobWithOutput, TranslationRequest,
    TranslationState, TranslationUnit,
};
use crate::onshape::models::{TranslationJob, TranslationResolution};

const BASE_URL: &str = "https://cad.onshape.com/api";

type HmacSha256 = Hmac<Sha256>;

pub struct OnShapeClient {
    pub http_client: reqwest::blocking::Client,
    rate_limiter: DefaultDirectRateLimiter,
    access_key: String,
    secret_key: String,
}

impl OnShapeClient {
    pub fn new(
        access_key: String,
        secret_key: String,
        proxy_url: Option<Url>,
    ) -> Result<Self> {
        Ok(Self {
            http_client: {
                let mut b = ClientBuilder::new().gzip(true).redirect(Policy::none());
                if let Some(proxy_url) = proxy_url {
                    b = b
                        .proxy(Proxy::all(proxy_url)?)
                        .danger_accept_invalid_certs(true);
                }
                b.build()?
            },
            rate_limiter: RateLimiter::direct(Quota::per_second(nonzero!(4u32))),
            access_key: access_key,
            secret_key: secret_key,
        })
    }

    pub fn get_document_elements(
        &self,
        document_id: &String,
        workspace_id: &String,
    ) -> Result<HashMap<String, DocumentElement>> {
        let url = format!(
            "{}/documents/d/{document_id}/w/{workspace_id}/elements",
            BASE_URL,
            document_id = document_id,
            workspace_id = workspace_id
        );
        let elements: Vec<DocumentElement> = self.request(Method::GET, url).send()?.json()?;

        let mut elements_by_id = HashMap::new();
        for e in elements {
            elements_by_id.insert(e.id.clone(), e);
        }
        Ok(elements_by_id)
    }

    pub fn get_studio_parts(
        &self,
        document_id: &String,
        workspace_id: &String,
        part_studio_id: &String,
    ) -> Result<Vec<Part>> {
        Ok(self
            .get_studio_parts_internal(document_id, workspace_id, part_studio_id)?
            .json()?)
    }

    pub fn get_studio_parts_json(
        &self,
        document_id: &String,
        workspace_id: &String,
        part_studio_id: &String,
    ) -> Result<String> {
        Ok(self
            .get_studio_parts_internal(document_id, workspace_id, part_studio_id)?
            .text()?)
    }

    fn get_studio_parts_internal(
        &self,
        document_id: &String,
        workspace_id: &String,
        part_studio_id: &String,
    ) -> Result<Response> {
        let url = format!(
            "{}/parts/d/{document_id}/w/{workspace_id}/e/{part_studio_id}",
            BASE_URL,
        );

        let res = self.request(Method::GET, url).send()?;
        Ok(res)
    }

    pub fn get_part_stl(
        &self,
        document_id: &String,
        workspace_id: &String,
        element_id: &String,
        part_id: &String,
    ) -> Result<String> {
        let mut url = Url::from_str(&format!(
            "{}/parts/d/{document_id}/w/{microversion_id}/e/{element_id}/partid/{part_id}/stl?",
            BASE_URL,
            document_id = document_id,
            microversion_id = workspace_id,
            element_id = element_id,
            part_id = part_id,
        ))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("mode", "text");
            query.append_pair("units", "millimeter");
            query.append_pair("angleTolerance", "0.04363323129985824");
            query.append_pair("chordTolerance", "0.06");
            query.append_pair("minFacetWidth", "0.025");
            query.append_pair("configuration", "");
        }

        let res = self.request(Method::GET, url).send()?;
        assert!(res.status().is_redirection(), "Redirect expected");

        let redirect_url = res
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()?;
        Ok(self.request(Method::GET, redirect_url).send()?.text()?)
    }

    pub fn get_part_parasolid(
        &self,
        document_id: &String,
        microversion_id: &String,
        element_id: &String,
        part_id: &String,
        configuration: &String,
    ) -> Result<String> {
        let mut url = Url::from_str(&format!(
            "{}/parts/d/{document_id}/m/{microversion_id}/e/{element_id}/partid/{part_id}/parasolid?",
            BASE_URL,
            document_id = document_id,
            microversion_id = microversion_id,
            element_id = element_id,
            part_id = part_id,
        ))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("version", "35.1");
            query.append_pair("includeExportIds", "true");
            query.append_pair("binaryExport", "false");
            query.append_pair("configuration", configuration);
        }

        let res = self.request(Method::GET, url).send()?;
        assert!(res.status().is_redirection(), "Redirect expected");

        let redirect_url = res
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()?;

        let para_text = self.request(Method::GET, redirect_url).send()?.text()?;

        lazy_static! {
            // DATE=2023-06-22T10:00:01 (UTC);
            static ref HEADER_DATE_PATTERN: Regex = Regex::new(r"(?m)DATE=.*$\n").unwrap();
        }

        Ok(HEADER_DATE_PATTERN.replace(&para_text, "").into())
    }

    pub fn begin_translation(
        &self,
        format: &ExportFileFormat,
        document_id: &String,
        workspace_id: &String,
        element_id: &String,
        part_id: &String,
        basename: &String,
    ) -> Result<TranslationJobWithOutput> {
        let output_filename =
            format!("{basename}.{extension}", extension = format.extension());

        let url = Url::from_str(&format!(
            "{}/partstudios/d/{document_id}/w/{workspace_id}/e/{element_id}/translations",
            BASE_URL,
        ))?;
        let req = self.request(Method::POST, url);
        let payload = TranslationRequest {
            part_ids: part_id.into(),
            destination_name: output_filename.clone(),
            format: format.clone(),
            configuration: "".into(),
            store_in_document: false,
            resolution: TranslationResolution::Fine,

            distance_tolerance: 0.00006,
            angular_tolerance: 0.04363323129985824,
            maximum_chord_length: 10.,
            specify_units: true,
            units: TranslationUnit::Millimeters,

            image_width: 96,
            image_height: 96,
        };

        let res = req.json(&payload).send()?;
        let job: TranslationJob = res.json()?;
        Ok(TranslationJobWithOutput {
            job,
            output_filename: Utf8PathBuf::from_str(&output_filename.clone()).unwrap(),
            format: format.clone(),
        })
    }

    pub fn check_translation(
        &self,
        job: &TranslationJobWithOutput,
    ) -> Result<TranslationJobWithOutput> {
        let j: TranslationJob = self.request(Method::GET, job.url.clone()).send()?.json()?;
        Ok(TranslationJobWithOutput {
            job: j,
            output_filename: job.output_filename.clone(),
            format: job.format.clone(),
        })
    }

    pub fn download_translated_file(&self, job: &TranslationJobWithOutput) -> Result<Bytes> {
        let url = match (job.request_state, job.result_external_data_ids.as_deref()) {
            (TranslationState::Done, Some([external_id, ..])) => Url::from_str(&format!(
                "{}/documents/d/{document_id}/externaldata/{external_id}",
                BASE_URL,
                document_id = job.document_id,
            ))?,
            _ => {
                return Err(anyhow!(
                    "Job is not in a state where its file can be downloaded, {:#?}",
                    job
                ))
            }
        };

        eprintln!("Downloading file, {}", job.output_filename);
        let res = self.request(Method::GET, url).send()?;
        Ok(res.bytes()?)
    }

    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        // TODO(shyndman): This works for now, but ideally should be refactored to:
        //     1. perform rate limiting when the request is SENT
        //     2. handle retries when the server rate limits a request
        loop {
            match self.rate_limiter.check() {
                Ok(_) => break,
                Err(negative) => {
                    let wait_duration = negative.wait_time_from(QuantaClock::default().now());
                    // eprintln!("Rate limiting for {}ms", wait_duration.as_millis());
                    std::thread::sleep(wait_duration);
                }
            }
        }

        let url = url.into_url().expect("Could not convert to URL");
        let content_type = mime::APPLICATION_JSON;

        // Prepare the signature
        let nonce = create_nonce();
        let date = httpdate::fmt_http_date(SystemTime::now());
        let path = url.path();
        let query: String = url.query().map_or("".into(), |val| {
            percent_encoding::percent_decode_str(val)
                .decode_utf8()
                .expect("Error parsing query")
                .into_owned()
        });

        let signature_plaintext =
            // NOTE: While not documented, the trailing newline is a requirement
            format!("{method}\n{nonce}\n{date}\n{content_type}\n{path}\n{query}\n")
                .to_lowercase();

        let mac = {
            let mut m = HmacSha256::new_from_slice(self.secret_key.as_bytes())
                .expect("HMAC can take key of any size");
            m.update(signature_plaintext.as_bytes());
            m
        };

        let authorization_val = format!(
            "On {access_key}:HmacSHA256:{signature}",
            access_key = self.access_key,
            // NOTE: The OnShape API requires that the signature be encoded as base64 with
            // padding characters, and as such, we use the STANDARD engine (not the
            // STANDARD_NO_PAD).
            signature =
                base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
        );

        self.http_client
            .request(method, url)
            .header(header::AUTHORIZATION, authorization_val)
            .header(
                header::ACCEPT,
                "application/vnd.onshape.v2+json;charset=UTF-8;qs=0.2",
            )
            .header(header::CONTENT_TYPE, content_type.to_string())
            .header(header::DATE, date)
            .header("On-Nonce", nonce)
    }
}

fn create_nonce() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(25)
        .map(char::from)
        .collect()
}
