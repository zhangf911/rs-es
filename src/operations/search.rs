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

use hyper::status::StatusCode;

use rustc_serialize::Decodable;
use rustc_serialize::json::{Json, ToJson};

use ::Client;
use ::error::EsError;
use ::query::Query;
use ::util::StrJoin;
use super::common::Options;
use super::decode_json;
use super::format_indexes_and_types;
use super::format_query_string;
use super::ShardCountResult;

/// Search API using a query string
pub struct SearchURIOperation<'a, 'b> {
    /// The HTTP client
    client: &'a mut Client,

    /// The indexes to which this query applies
    indexes: &'b [&'b str],

    /// The types to which this query applies
    doc_types: &'b [&'b str],

    /// Optional options
    options: Options<'b>
}

/// Options for the various search_type parameters
pub enum SearchType {
    DFSQueryThenFetch,
    DFSQueryAndFetch,
    QueryThenFetch,
    QueryAndFetch
}

impl ToString for SearchType {
    fn to_string(&self) -> String {
        match self {
            &SearchType::DFSQueryThenFetch => "dfs_query_then_fetch",
            &SearchType::DFSQueryAndFetch  => "dfs_query_and_fetch",
            &SearchType::QueryThenFetch    => "query_then_fetch",
            &SearchType::QueryAndFetch     => "query_and_fetch"
        }.to_string()
    }
}

impl<'a, 'b> SearchURIOperation<'a, 'b> {
    pub fn new(client: &'a mut Client) -> SearchURIOperation<'a, 'b> {
        SearchURIOperation {
            client:    client,
            indexes:   &[],
            doc_types: &[],
            options:   Options::new()
        }
    }

    pub fn with_indexes(&'b mut self, indexes: &'b [&'b str]) -> &'b mut Self {
        self.indexes = indexes;
        self
    }

    pub fn with_types(&'b mut self, doc_types: &'b [&'b str]) -> &'b mut Self {
        self.doc_types = doc_types;
        self
    }

    pub fn with_query(&'b mut self, qs: String) -> &'b mut Self {
        self.options.push(("q", qs));
        self
    }

    add_option!(with_df, "df");
    add_option!(with_analyzer, "analyzer");
    add_option!(with_lowercase_expanded_terms, "lowercase_expanded_terms");
    add_option!(with_analyze_wildcard, "analyze_wildcard");
    add_option!(with_default_operator, "default_operator");
    add_option!(with_lenient, "lenient");
    add_option!(with_explain, "explain");
    add_option!(with_source, "_source");
    add_option!(with_sort, "sort");
    add_option!(with_routing, "routing");
    add_option!(with_track_scores, "track_scores");
    add_option!(with_timeout, "timeout");
    add_option!(with_terminate_after, "terminate_after");
    add_option!(with_from, "from");
    add_option!(with_size, "size");
    add_option!(with_search_type, "search_type");

    pub fn with_fields(&'b mut self, fields: &[&str]) -> &'b mut Self {
        self.options.push(("fields", fields.iter().join(",")));
        self
    }

    pub fn send(&'b mut self) -> Result<SearchResult, EsError> {
        let url = format!("/{}/_search{}",
                          format_indexes_and_types(&self.indexes, &self.doc_types),
                          format_query_string(&self.options));
        info!("Searching with: {}", url);
        let (status_code, result) = try!(self.client.get_op(&url));
        info!("Search result (status: {}, result: {:?})", status_code, result);
        match status_code {
            StatusCode::Ok => Ok(SearchResult::from(&result.unwrap())),
            _              => Err(EsError::EsError(format!("Unexpected status: {}", status_code)))
        }
    }
}

struct SearchQueryOperationBody<'b> {
    /// The query
    query: Option<&'b Query>,

    /// Timeout
    timeout: Option<&'b str>,

    /// From
    from: i64,

    /// Size
    size: i64,

    /// Terminate early (marked as experimental in the ES docs)
    terminate_after: Option<i64>,

    /// Stats groups to which the query belongs
    stats: Option<Vec<String>>,

    /// Minimum score to use
    min_score: Option<f64>
}

impl<'a> ToJson for SearchQueryOperationBody<'a> {
    fn to_json(&self) -> Json {
        let mut d = BTreeMap::new();
        d.insert("from".to_string(), self.from.to_json());
        d.insert("size".to_string(), self.size.to_json());
        optional_add!(d, self.query, "query");
        optional_add!(d, self.timeout, "timeout");
        optional_add!(d, self.terminate_after, "terminate_after");
        optional_add!(d, self.stats, "stats");
        optional_add!(d, self.min_score, "min_score");
        Json::Object(d)
    }
}

/// Search API using a Query DSL body
pub struct SearchQueryOperation<'a, 'b> {
    /// The HTTP client
    client: &'a mut Client,

    /// The indexes to which this query applies
    indexes: &'b [&'b str],

    /// The types to which the query applies
    doc_types: &'b [&'b str],

    /// Optionals
    options: Options<'b>,

    /// The query body
    body: SearchQueryOperationBody<'b>
}

impl <'a, 'b> SearchQueryOperation<'a, 'b> {
    pub fn new(client: &'a mut Client) -> SearchQueryOperation<'a, 'b> {
        SearchQueryOperation {
            client:    client,
            indexes:   &[],
            doc_types: &[],
            options:   Options::new(),
            body:      SearchQueryOperationBody {
                query:           None,
                timeout:         None,
                from:            0,
                size:            10,
                terminate_after: None,
                stats:           None,
                min_score:       None
            }
        }
    }

    pub fn with_indexes(&'b mut self, indexes: &'b [&'b str]) -> &'b mut Self {
        self.indexes = indexes;
        self
    }

    pub fn with_types(&'b mut self, doc_types: &'b [&'b str]) -> &'b mut Self {
        self.doc_types = doc_types;
        self
    }

    pub fn with_query(&'b mut self, query: &'b Query) -> &'b mut Self {
        self.body.query = Some(query);
        self
    }

    pub fn with_timeout(&'b mut self, timeout: &'b str) -> &'b mut Self {
        self.body.timeout = Some(timeout);
        self
    }

    pub fn with_from(&'b mut self, from: i64) -> &'b mut Self {
        self.body.from = from;
        self
    }

    pub fn with_size(&'b mut self, size: i64) -> &'b mut Self {
        self.body.size = size;
        self
    }

    pub fn with_terminate_after(&'b mut self, terminate_after: i64) -> &'b mut Self {
        self.body.terminate_after = Some(terminate_after);
        self
    }

    pub fn with_stats<S>(&'b mut self, stats: &[S]) -> &'b mut Self
        where S: ToString
    {
        self.body.stats = Some(stats.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn with_min_score(&'b mut self, min_score: f64) -> &'b mut Self {
        self.body.min_score = Some(min_score);
        self
    }

    add_option!(with_routing, "routing");
    add_option!(with_search_type, "search_type");
    add_option!(with_query_cache, "query_cache");

    pub fn send(&'b mut self) -> Result<SearchResult, EsError> {
        let url = format!("/{}/_search{}",
                          format_indexes_and_types(&self.indexes, &self.doc_types),
                          format_query_string(&self.options));
        let (status_code, result) = try!(self.client.post_body_op(&url, &self.body.to_json()));
        match status_code {
            StatusCode::Ok => Ok(SearchResult::from(&result.unwrap())),
            _              => Err(EsError::EsError(format!("Unexpected status: {}", status_code)))
        }
    }
}

#[derive(Debug)]
pub struct SearchHitsHitsResult {
    pub index:    String,
    pub doc_type: String,
    pub id:       String,
    pub score:    f64,
    pub source:   Option<Json>,
    pub fields:   Option<Json>
}

impl SearchHitsHitsResult {
    /// Get the source document as a struct, the raw JSON version is available
    /// directly from the source field
    pub fn source<T: Decodable>(self) -> Result<T, EsError> {
        match self.source {
            Some(source) => decode_json(source),
            None         => Err(EsError::EsError("No source field".to_string()))
        }
    }
}

impl<'a> From<&'a Json> for SearchHitsHitsResult {
    fn from(r: &'a Json) -> SearchHitsHitsResult {
        SearchHitsHitsResult {
            index:    get_json_string!(r, "_index"),
            doc_type: get_json_string!(r, "_type"),
            id:       get_json_string!(r, "_id"),
            score:    get_json_f64!(r, "_score"),
            source:   r.find("_source").map(|s| s.clone()),
            fields:   r.find("fields").map(|s| s.clone())
        }
    }
}

pub struct SearchHitsResult {
    pub total: i64,
    pub hits:  Vec<SearchHitsHitsResult>
}

impl<'a> From<&'a Json> for SearchHitsResult {
    fn from(r: &'a Json) -> SearchHitsResult {
        SearchHitsResult {
            total: get_json_i64!(r, "total"),
            hits:  r.find("hits")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|j| SearchHitsHitsResult::from(j))
                .collect()
        }
    }
}

pub struct SearchResult {
    pub shards: ShardCountResult,
    pub hits:   SearchHitsResult
}

impl<'a> From<&'a Json> for SearchResult {
    fn from(r: &'a Json) -> SearchResult {
        SearchResult {
            shards: decode_json(r.find("_shards")
                                .unwrap()
                                .clone()).unwrap(),
            hits:   SearchHitsResult::from(r.find("hits")
                                           .unwrap())
        }
    }
}
