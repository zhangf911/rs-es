/*
 * Copyright 2015 Ben Ashford
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::BTreeMap;
use std::collections::HashMap;

use hyper::status::StatusCode;

use rustc_serialize::json::{Json, ToJson};

use ::Client;
use ::error::EsError;
use ::query::Query;
use super::common::Options;
use super::decode_json;
use super::format_indexes_and_types;
use super::format_query_string;
use super::ShardCountResult;

/// An ES DELETE operation for a specific document
pub struct DeleteOperation<'a, 'b> {
    /// The HTTP client
    client:   &'a mut Client,

    /// The index
    index:    &'b str,

    /// The type
    doc_type: &'b str,

    /// The ID
    id:       &'b str,

    /// Optional options
    options:  Options<'b>
}

impl<'a, 'b> DeleteOperation<'a, 'b> {
    pub fn new(client:   &'a mut Client,
               index:    &'b str,
               doc_type: &'b str,
               id:       &'b str) -> DeleteOperation<'a, 'b> {
        DeleteOperation {
            client:   client,
            index:    index,
            doc_type: doc_type,
            id:       id,
            options:  Options::new()
        }
    }

    add_option!(with_version, "version");
    add_option!(with_routing, "routing");
    add_option!(with_parent, "parent");
    add_option!(with_consistency, "consistency");
    add_option!(with_refresh, "refresh");
    add_option!(with_timeout, "timeout");

    pub fn send(&'a mut self) -> Result<DeleteResult, EsError> {
        let url = format!("/{}/{}/{}{}",
                          self.index,
                          self.doc_type,
                          self.id,
                          format_query_string(&mut self.options));
        let (status_code, result) = try!(self.client.delete_op(&url));
        info!("DELETE OPERATION STATUS: {:?} RESULT: {:?}", status_code, result);
        match status_code {
            StatusCode::Ok =>
                Ok(DeleteResult::from(&result.unwrap())),
            _ =>
                Err(EsError::EsError(format!("Unexpected status: {}", status_code)))
        }
    }
}

struct DeleteByQueryBody<'a> {
    query: &'a Query
}

impl<'a> ToJson for DeleteByQueryBody<'a> {
    fn to_json(&self) -> Json {
        let mut d = BTreeMap::new();
        d.insert("query".to_string(), self.query.to_json());
        Json::Object(d)
    }
}

enum QueryOption<'a> {
    String(String),
    Document(DeleteByQueryBody<'a>)
}

/// Delete-by-query API.
///
/// The query can be specified either as a String as a query parameter or in the
/// body using the Query DSL.
pub struct DeleteByQueryOperation<'a, 'b> {
    /// The HTTP client
    client:    &'a mut Client,

    /// The indexes to which this query apply
    indexes:   &'b [&'b str],

    /// The types to which this query applies
    doc_types: &'b [&'b str],

    /// The query itself, either in parameter or Query DSL form.
    query:     QueryOption<'b>,

    /// Optional options
    options:   Options<'b>
}

impl<'a, 'b> DeleteByQueryOperation<'a, 'b> {
    pub fn new(client: &'a mut Client) -> DeleteByQueryOperation<'a, 'b> {
        DeleteByQueryOperation {
            client:    client,
            indexes:   &[],
            doc_types: &[],
            query:     QueryOption::String("".to_string()),
            options:   Options::new()
        }
    }

    pub fn with_indexes(&'b mut self, indexes: &'b [&'b str]) -> &'b mut Self {
        self.indexes = indexes;
        self
    }

    pub fn with_doc_types(&'b mut self, doc_types: &'b [&'b str]) -> &'b mut Self {
        self.doc_types = doc_types;
        self
    }

    pub fn with_query_string(&'b mut self, qs: String) -> &'b mut Self {
        self.query = QueryOption::String(qs);
        self
    }

    pub fn with_query(&'b mut self, q: &'b Query) -> &'b mut Self {
        self.query = QueryOption::Document(DeleteByQueryBody { query: q });
        self
    }

    add_option!(with_df, "df");
    add_option!(with_analyzer, "analyzer");
    add_option!(with_default_operator, "default_operator");
    add_option!(with_routing, "routing");
    add_option!(with_consistency, "consistency");

    pub fn send(&'a mut self) -> Result<Option<DeleteByQueryResult>, EsError> {
        let options = match &self.query {
            &QueryOption::Document(_)   => &mut self.options,
            &QueryOption::String(ref s) => {
                let opts = &mut self.options;
                opts.push(("q", s.clone()));
                opts
            }
        };
        let url = format!("/{}/_query{}",
                          format_indexes_and_types(&self.indexes, &self.doc_types),
                          format_query_string(options));
        let (status_code, result) = try!(match self.query {
            QueryOption::Document(ref d) => self.client.delete_body_op(&url,
                                                                       &d.to_json()),
            QueryOption::String(_)       => self.client.delete_op(&url)
        });
        info!("DELETE BY QUERY STATUS: {:?}, RESULT: {:?}", status_code, result);
        match status_code {
            StatusCode::Ok =>
                Ok(Some(DeleteByQueryResult::from(&result.unwrap()))),
            StatusCode::NotFound =>
                Ok(None),
            _  =>
                Err(EsError::EsError(format!("Unexpected status: {}", status_code)))
        }
    }
}

/// Result of a DELETE operation
#[derive(Debug)]
pub struct DeleteResult {
    pub found:    bool,
    pub index:    String,
    pub doc_type: String,
    pub id:       String,
    pub version:  i64
}

/// This is required because the JSON keys do not match the struct
impl<'a> From<&'a Json> for DeleteResult {
    fn from(r: &'a Json) -> DeleteResult {
        DeleteResult {
            found:    get_json_bool!(r, "found"),
            index:    get_json_string!(r, "_index"),
            doc_type: get_json_string!(r, "_type"),
            id:       get_json_string!(r, "_id"),
            version:  get_json_i64!(r, "_version")
        }
    }
}

#[derive(Debug)]
pub struct DeleteByQueryIndexResult {
    pub shards: ShardCountResult
}

impl DeleteByQueryIndexResult {
    fn successful(&self) -> bool {
        self.shards.failed == 0
    }
}

// Required because of change in names of keys
impl<'a> From<&'a Json> for DeleteByQueryIndexResult {
    fn from(r: &'a Json) -> DeleteByQueryIndexResult {
        info!("Parsing DeleteByQueryIndexResult: {:?}", r);
        DeleteByQueryIndexResult {
            shards: decode_json(r.find("_shards").unwrap().clone()).unwrap()
        }
    }
}

/// The result of a Delete-by-query request
#[derive(Debug)]
pub struct DeleteByQueryResult {
    pub indices: HashMap<String, DeleteByQueryIndexResult>
}

impl DeleteByQueryResult {
    pub fn successful(&self) -> bool {
        for dbqir in self.indices.values() {
            if !dbqir.successful() {
                return false
            }
        }
        true
    }
}

// Required because of JSON structure and keys
impl<'a> From<&'a Json> for DeleteByQueryResult {
    fn from(r: &'a Json) -> DeleteByQueryResult {
        info!("DeleteByQueryResult from: {:?}", r);

        let indices = r.find("_indices").unwrap().as_object().unwrap();
        let mut indices_map = HashMap::new();
        for (k, v) in indices {
            indices_map.insert(k.clone(), DeleteByQueryIndexResult::from(v));
        }
        DeleteByQueryResult {
            indices: indices_map
        }
    }
}
