use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    RoomsRead,
    RoomsWrite,
    ZonesRead,
    ZonesWrite,
    StatsRead,
    UserRead,
    UserWrite,
    Admin,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::RoomsRead => "rooms:read",
            Scope::RoomsWrite => "rooms:write",
            Scope::ZonesRead => "zones:read",
            Scope::ZonesWrite => "zones:write",
            Scope::StatsRead => "stats:read",
            Scope::UserRead => "user:read",
            Scope::UserWrite => "user:write",
            Scope::Admin => "admin",
        }
    }

    pub fn all() -> Vec<Scope> {
        vec![
            Scope::RoomsRead,
            Scope::RoomsWrite,
            Scope::ZonesRead,
            Scope::ZonesWrite,
            Scope::StatsRead,
            Scope::UserRead,
            Scope::UserWrite,
            Scope::Admin,
        ]
    }

    pub fn default_scopes() -> Vec<Scope> {
        vec![
            Scope::RoomsRead,
            Scope::RoomsWrite,
            Scope::ZonesRead,
            Scope::ZonesWrite,
            Scope::StatsRead,
            Scope::UserRead,
        ]
    }
}

impl FromStr for Scope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rooms:read" => Ok(Scope::RoomsRead),
            "rooms:write" => Ok(Scope::RoomsWrite),
            "zones:read" => Ok(Scope::ZonesRead),
            "zones:write" => Ok(Scope::ZonesWrite),
            "stats:read" => Ok(Scope::StatsRead),
            "user:read" => Ok(Scope::UserRead),
            "user:write" => Ok(Scope::UserWrite),
            "admin" => Ok(Scope::Admin),
            _ => Err(format!("Unknown scope: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScopeSet(HashSet<Scope>);

impl ScopeSet {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    pub fn from_vec(scopes: Vec<Scope>) -> Self {
        Self(scopes.into_iter().collect())
    }

    pub fn from_string(scopes_str: &str) -> Result<Self, String> {
        let scopes: Result<Vec<Scope>, _> = scopes_str
            .split_whitespace()
            .map(Scope::from_str)
            .collect();
        Ok(Self::from_vec(scopes?))
    }

    pub fn to_string(&self) -> String {
        self.0
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn to_json_array(&self) -> serde_json::Value {
        serde_json::json!(self.0.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    }

    pub fn from_json_array(json: &serde_json::Value) -> Result<Self, String> {
        let array = json.as_array().ok_or("Expected array")?;
        let scopes: Result<Vec<Scope>, String> = array
            .iter()
            .map(|v| v.as_str().ok_or_else(|| "Expected string".to_string()).and_then(Scope::from_str))
            .collect();
        Ok(Self::from_vec(scopes?))
    }

    pub fn contains(&self, scope: &Scope) -> bool {
        self.0.contains(scope) || self.0.contains(&Scope::Admin)
    }

    pub fn has_any(&self, required: &[Scope]) -> bool {
        if self.0.contains(&Scope::Admin) {
            return true;
        }
        required.iter().any(|s| self.0.contains(s))
    }

    pub fn insert(&mut self, scope: Scope) {
        self.0.insert(scope);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Scope> {
        self.0.iter()
    }
}

impl Default for ScopeSet {
    fn default() -> Self {
        Self::new()
    }
}