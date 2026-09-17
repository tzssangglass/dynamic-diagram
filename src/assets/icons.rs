//! Icon lookup — multiple tiers in one table:
//!   1. Material Symbols (3,912 filled, outlined style) — Apache 2.0, © Google
//!   2. Cloud provider official icons (AWS/Azure/GCP/Cloudflare) — full-color
//!      SVGs embedded as fragments; trademarks of their owners, for diagrams
//!   3. Software-engineering structural glyphs live in svg.rs (hand-drawn)
//!
//! Data format (material-symbols.txt):  `<name>\t<path d>`
//! Data format (cloud-icons.txt):       `<name>\t<viewBox>\t<inner svg>`

use std::collections::HashMap;
use std::sync::OnceLock;

const MATERIAL: &str = include_str!("material-symbols.txt");
const CLOUD: &str = include_str!("cloud-icons.txt");
const TABLER: &str = include_str!("tabler-icons.txt");

pub enum Icon {
    /// single ink-filled path, 960x960 grid y-negative-up (Material Symbols)
    Path(&'static str),
    /// stroke-style path, 24x24 viewbox (Tabler): render fill=none + stroke
    Stroke(&'static str),
    /// full-color embedded SVG fragment: (viewBox, inner content)
    Svg(&'static str, &'static str),
}

fn material_table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = HashMap::with_capacity(4000);
        for line in MATERIAL.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((name, path)) = line.split_once('\t') {
                m.insert(name, path);
            }
        }
        m
    })
}

fn cloud_table() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    static T: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = HashMap::with_capacity(3000);
        for line in CLOUD.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut it = line.splitn(3, '\t');
            let (Some(name), Some(vb), Some(inner)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            m.insert(name, (vb, inner));
        }
        m
    })
}

fn tabler_table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = HashMap::with_capacity(5200);
        for line in TABLER.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((name, path)) = line.split_once('\t') {
                m.insert(name, path);
            }
        }
        m
    })
}

/// icon name -> Icon. None if unknown. Lookup order: cloud (namespaced
/// aws/…, azure/…, gcp/…, cf/…) → material → tabler ("ti/<name>" forces tabler).
// common-service short names -> canonical provider icon names
const CLOUD_ALIASES: &[(&str, &str)] = &[
    ("ec2", "aws/resource-compute-amazon-ec2-instance"),
    ("s3", "aws/resource-storage-amazon-simple-storage-service-bucket"),
    ("lambda", "aws/resource-compute-aws-lambda-lambda-function"),
    ("dynamodb", "aws/resource-database-amazon-dynamodb-table"),
    ("cloudfront", "aws/resource-networking-content-delivery-amazon-cloudfront-edge-location"),
    ("route53", "aws/resource-networking-content-delivery-amazon-route-53-hosted-zone"),
    ("vpc", "aws/resource-networking-content-delivery-amazon-vpc-virtual-private-cloud-vpc"),
    ("sqs", "aws/resource-application-integration-amazon-simple-queue-service-queue"),
    ("sns", "aws/resource-application-integration-amazon-simple-notification-service-topic"),
    ("iam", "aws/service-security-identity-compliance-16-aws-iam-identity-center"),
    ("azure_vm", "azure/compute-virtual-machine"),
    ("bigquery", "gcp/bigquery-bigquery"),
    ("cloud_run", "gcp/cloud-run-cloud-run"),
    ("gce", "gcp/compute-engine-compute-engine"),
    ("cloudflare", "cf/logo"),
    ("workers", "cf/workers"),
    ("r2", "cf/r2"),
];

pub fn get(name: &str) -> Option<Icon> {
    let name = CLOUD_ALIASES
        .iter()
        .find(|(short, _)| *short == name)
        .map(|(_, full)| *full)
        .unwrap_or(name);
    if let Some((vb, inner)) = cloud_table().get(name) {
        return Some(Icon::Svg(vb, inner));
    }
    if let Some(n) = name.strip_prefix("ti/") {
        return tabler_table().get(n).copied().map(Icon::Stroke);
    }
    if let Some(d) = material_table().get(name) {
        return Some(Icon::Path(d));
    }
    // fall through to tabler for names material lacks
    tabler_table().get(name).copied().map(Icon::Stroke)
}

/// all icon names, prefixed by source: material/cloud
pub fn all() -> Vec<String> {
    let mut v: Vec<String> = material_table().keys().map(|k| k.to_string()).collect();
    v.extend(cloud_table().keys().map(|k| k.to_string()));
    v.extend(tabler_table().keys().map(|k| format!("ti/{k}")));
    v.sort_unstable();
    v
}
