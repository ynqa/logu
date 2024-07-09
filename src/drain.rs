use std::{collections::HashMap, fmt::Debug};

use lru::LruCache;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LogCluster {
    log_template_tokens: Vec<String>,
    cluster_id: usize,
    size: usize,
}

impl LogCluster {
    pub fn get_template(&self) -> String {
        self.log_template_tokens.join(" ")
    }
}

#[derive(Clone, Default, Debug)]
pub struct Node {
    key_to_child_node: HashMap<String, Node>,
    cluster_ids: Vec<usize>,
}

pub struct Drain {
    id_to_cluster: LruCache<usize, LogCluster>,

    log_cluster_depth: usize,
    max_node_depth: usize,

    /// Similarity threshold.
    /// A new log cluster will be created
    /// if the similarity of tokens for log message is below this.
    sim_th: f32,

    /// Maximum number of children within a node.
    max_children: usize,

    /// Maximum number of clusters.
    max_clusters: Option<usize>,
    cluster_counter: usize,

    root: Node,

    param_str: String,
}

impl Default for Drain {
    fn default() -> Self {
        Self {
            id_to_cluster: LruCache::unbounded(),
            log_cluster_depth: 4,
            max_node_depth: 4 - 2,
            sim_th: 0.4,
            max_children: 100,
            max_clusters: None,
            cluster_counter: 0,
            root: Node::default(),
            param_str: "<*>".to_string(),
        }
    }
}

impl Drain {
    pub fn clusters(&self) -> Vec<&LogCluster> {
        self.id_to_cluster.iter().map(|(_, v)| v).collect()
    }

    pub fn train<T: AsRef<str>>(&mut self, log_message: T) -> LogCluster {
        let tokens = tokenize(log_message.as_ref());
        match self.tree_search(&tokens, self.sim_th, false) {
            Some(mut match_cluster) => {
                match_cluster.log_template_tokens =
                    self.create_template(&tokens, &match_cluster.log_template_tokens);
                match_cluster.size += 1;
                self.id_to_cluster
                    .put(match_cluster.cluster_id, match_cluster.clone());
                match_cluster
            }
            None => {
                self.cluster_counter += 1;
                let mut match_cluster = LogCluster {
                    log_template_tokens: tokens,
                    cluster_id: self.cluster_counter,
                    size: 1,
                };
                self.id_to_cluster
                    .put(match_cluster.cluster_id, match_cluster.clone());
                self.add_seq_to_prefix_tree(&mut match_cluster);
                match_cluster
            }
        }
    }

    fn tree_search(
        &mut self,
        tokens: &[String],
        sim_th: f32,
        include_params: bool,
    ) -> Option<LogCluster> {
        let token_count = tokens.len();

        let mut cur_node = self.root.key_to_child_node.get(&token_count.to_string())?;
        if token_count == 0 {
            return self.id_to_cluster.get(&cur_node.cluster_ids[0]).cloned();
        }

        let mut cur_node_depth = 1;
        for token in tokens {
            // At max depth.
            if cur_node_depth == self.max_node_depth {
                break;
            }

            // At last token.
            if cur_node_depth == token_count {
                break;
            }

            cur_node = cur_node
                .key_to_child_node
                .get(token)
                .or_else(|| cur_node.key_to_child_node.get(&self.param_str))?;

            cur_node_depth += 1;
        }
        self.fast_match(
            &cur_node.cluster_ids.clone(),
            tokens,
            sim_th,
            include_params,
        )
    }

    fn fast_match(
        &mut self,
        cluster_ids: &[usize],
        tokens: &[String],
        sim_th: f32,
        include_params: bool,
    ) -> Option<LogCluster> {
        let mut match_cluster = None;
        let mut max_cluster = None;

        let mut max_sim = -1.0;
        let mut max_param_count = -1;
        for id in cluster_ids {
            let cluster = self.id_to_cluster.get(&id).cloned();
            if let Some(cluster) = cluster {
                let (cur_sim, param_count) =
                    self.get_seq_distance(tokens, &cluster.log_template_tokens, include_params);
                if cur_sim > max_sim || (cur_sim == max_sim && param_count > max_param_count) {
                    max_sim = cur_sim;
                    max_param_count = param_count;
                    max_cluster = Some(cluster);
                }
            }
        }
        if max_sim >= sim_th {
            match_cluster = max_cluster;
        }
        match_cluster
    }

    fn get_seq_distance(
        &self,
        seq1: &[String],
        seq2: &[String],
        include_params: bool,
    ) -> (f32, isize) {
        let mut sim_tokens = 0;
        let mut param_count = 0;

        for (token1, token2) in seq1.iter().zip(seq2.iter()) {
            if token1 == &self.param_str {
                param_count += 1;
            } else if token1 == token2 {
                sim_tokens += 1;
            }
        }
        if include_params {
            sim_tokens += param_count;
        }
        (sim_tokens as f32 / seq1.len() as f32, param_count)
    }

    fn add_seq_to_prefix_tree(&mut self, cluster: &mut LogCluster) {
        let token_count = cluster.log_template_tokens.len();
        let token_count_str = token_count.to_string();

        let mut cur_node: &mut Node = self
            .root
            .key_to_child_node
            .entry(token_count_str)
            .or_insert_with(Node::default);

        if token_count == 0 {
            cur_node.cluster_ids.push(cluster.cluster_id);
            return;
        }

        let mut current_depth = 1;
        for token in cluster.log_template_tokens.iter() {
            if current_depth >= self.max_node_depth || current_depth >= token_count {
                let mut new_cluster_ids = Vec::new();
                for cluster_id in cur_node
                    .cluster_ids
                    .iter()
                    .filter(|cluster_id| self.id_to_cluster.contains(cluster_id))
                {
                    new_cluster_ids.push(*cluster_id);
                }
                new_cluster_ids.push(cluster.cluster_id);
                cur_node.cluster_ids = new_cluster_ids;
                break;
            }

            if !cur_node.key_to_child_node.contains_key(token) {
                if !has_number(token) {
                    if cur_node.key_to_child_node.contains_key(&self.param_str) {
                        if cur_node.key_to_child_node.len() < self.max_children {
                            let new_node = Node::default();
                            cur_node.key_to_child_node.insert(token.clone(), new_node);
                            cur_node = cur_node.key_to_child_node.get_mut(token).unwrap();
                        } else {
                            cur_node = cur_node.key_to_child_node.get_mut(&self.param_str).unwrap();
                        }
                    } else {
                        if cur_node.key_to_child_node.len() + 1 < self.max_children {
                            let new_node = Node::default();
                            cur_node.key_to_child_node.insert(token.clone(), new_node);
                            cur_node = cur_node.key_to_child_node.get_mut(token).unwrap();
                        } else if cur_node.key_to_child_node.len() + 1 == self.max_children {
                            let new_node = Node::default();
                            cur_node
                                .key_to_child_node
                                .insert(self.param_str.clone(), new_node);
                            cur_node = cur_node.key_to_child_node.get_mut(&self.param_str).unwrap();
                        } else {
                            cur_node = cur_node.key_to_child_node.get_mut(&self.param_str).unwrap();
                        }
                    }
                } else {
                    if !cur_node.key_to_child_node.contains_key(&self.param_str) {
                        let new_node = Node::default();
                        cur_node
                            .key_to_child_node
                            .insert(self.param_str.clone(), new_node);
                        cur_node = cur_node.key_to_child_node.get_mut(&self.param_str).unwrap();
                    } else {
                        cur_node = cur_node.key_to_child_node.get_mut(&self.param_str).unwrap();
                    }
                }
            } else {
                cur_node = cur_node.key_to_child_node.get_mut(token).unwrap();
            }

            current_depth += 1;
        }
    }

    fn create_template(&self, seq1: &[String], seq2: &[String]) -> Vec<String> {
        let mut new_template_tokens = Vec::new();
        for (token1, token2) in seq1.iter().zip(seq2.iter()) {
            if token1 == token2 {
                new_template_tokens.push(token1);
            } else {
                new_template_tokens.push(&self.param_str);
            }
        }
        new_template_tokens.iter().map(|s| s.to_string()).collect()
    }
}

fn has_number(s: &str) -> bool {
    s.chars().any(|c| c.is_numeric())
}

fn tokenize(log_message: &str) -> Vec<String> {
    log_message
        .trim()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    mod train {
        use super::*;

        #[test]
        fn test() {
            let logs = vec![
                "connected to 10.0.0.1",
                "connected to 10.0.0.2",
                "connected to 10.0.0.3",
                "Hex number 0xDEADBEAF",
                "Hex number 0x10000",
                "user davidoh logged in",
                "user eranr logged in",
            ];
            let mut drain = Drain::default();
            for log in logs {
                drain.train(log);
            }
            let mut clusters = drain.clusters();
            clusters.sort_by_key(|c| c.cluster_id);
            assert_eq!(
                clusters,
                vec![
                    &LogCluster {
                        log_template_tokens: vec![
                            String::from("connected"),
                            String::from("to"),
                            String::from("<*>"),
                        ],
                        cluster_id: 1,
                        size: 3,
                    },
                    &LogCluster {
                        log_template_tokens: vec![
                            String::from("Hex"),
                            String::from("number"),
                            String::from("<*>"),
                        ],
                        cluster_id: 2,
                        size: 2,
                    },
                    &LogCluster {
                        log_template_tokens: vec![
                            String::from("user"),
                            String::from("<*>"),
                            String::from("logged"),
                            String::from("in"),
                        ],
                        cluster_id: 3,
                        size: 2,
                    },
                ]
            );
        }
    }
}
