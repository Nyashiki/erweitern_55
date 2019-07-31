use r#move::*;
use position::*;
use types::*;

use pyo3::prelude::*;
use numpy::PyArray1;

#[derive(Clone)]
pub struct Node {
    pub n: u32,
    pub v: f32,
    pub p: f32,
    pub w: f32,
    pub m: Move,
    pub parent: usize,
    pub children: std::vec::Vec<usize>,
    pub is_terminal: bool,
    pub virtual_loss: f32
}

impl Node {
    pub fn new(parent: usize, m: Move, policy: f32) -> Node {
        Node {
            n: 0,
            v: 0.0,
            p: policy,
            w: 0.0,
            m: m,
            parent: parent,
            children: Vec::new(),
            is_terminal: false,
            virtual_loss: 0.0
        }
    }

    pub fn get_puct(&self, parent_n: f32) -> f32 {
        const C_BASE: f32 = 19652.0;
        const C_INIT: f32 = 1.25;

        let c: f32 = ((1.0 + (self.n as f32) + C_BASE) / C_BASE).log2() + C_INIT;
        let q: f32 = if self.n as f32 + self.virtual_loss == 0.0 { 0.0 } else { 1.0 - (self.w + self.virtual_loss) / ((self.n as f32 + self.virtual_loss)) };
        let u: f32 = c * self.p * parent_n.sqrt() / (1.0 + (self.n as f32) + self.virtual_loss);

        return q + u;
    }

    pub fn expanded(&self) -> bool {
        return self.children.len() > 0 && !self.is_terminal;
    }
}

#[pyclass]
pub struct MCTS {
    pub game_tree: std::vec::Vec<Node>,
    pub node_count: usize
}

#[pymethods]
impl MCTS {
    #[new]
    pub fn new(obj: &PyRawObject) {
        obj.init(MCTS{
            game_tree: vec![Node::new(0, NULL_MOVE, 0.0); 1000000],
            node_count: 0
        });
    }

    pub fn set_root(&mut self) -> usize {
        for node in &mut self.game_tree {
            *node = Node::new(0, NULL_MOVE, 0.0);
        }

        self.node_count = 2;
        return 1;
    }

    pub fn best_move(&self, node: usize) -> Move {
        let best_child: usize = self.select_n_max_child(node);

        return self.game_tree[best_child].m;
    }

    pub fn print(&self, root: usize) {
        println!("playout: {}", self.game_tree[root].n);

        let best_child: usize = self.select_n_max_child(root);

        println!("N(s, a): {}", self.game_tree[best_child].n);
        println!("P(s, a): {}", self.game_tree[best_child].p);
        println!("V(s, a): {}", self.game_tree[best_child].v);
        println!("Q(s, a): {}", if self.game_tree[best_child].n == 0 { 0.0 } else { self.game_tree[best_child].w / self.game_tree[best_child].n as f32 });
    }

    pub fn select_leaf(&mut self, root_node: usize, position: &mut Position) -> usize {
        let mut node = root_node;

        loop {
            self.game_tree[node].virtual_loss += 1.0;

            if !self.game_tree[node].expanded() {
                break;
            }

            node = self.select_puct_max_child(node);
            position.do_move(&self.game_tree[node].m);
        }

        return node;
    }

    pub fn evaluate(&mut self, node: usize, position: &Position, np_policy: &PyArray1<f32>, mut value: f32) -> f32 {
        if self.game_tree[node].n > 0 {
            return self.game_tree[node].v;
        }

        let policy = np_policy.as_array();
        let mut legal_policy_sum: f32 = 0.0;

        let moves = position.generate_moves();

        for m in &moves {
            let index = m.to_policy_index();
            legal_policy_sum += policy[index];
        }

        let (is_repetition, is_check_repetition) = position.is_repetition();

        if is_repetition || moves.len() == 0 {
            self.game_tree[node].is_terminal = true;
        }

        // win or lose is determined by the game rule
        if self.game_tree[node].is_terminal {
            if is_check_repetition {
                value = 0.0;
            } else if is_repetition {
                value = if position.side_to_move == Color::White { 0.0 } else { 1.0 }
            } else {
                value = 0.0
            }
        }

        // set policy and vaue
        for m in &moves {
            let index = m.to_policy_index();

            self.game_tree[self.node_count] = Node::new(node, *m, policy[index] / legal_policy_sum);
            self.game_tree[node].children.push(self.node_count);
            self.node_count += 1;
        }
        self.game_tree[node].v = value;

        return value;
    }

    pub fn backpropagate(&mut self, leaf_node: usize, value: f32) {
        let mut node = leaf_node;
        let mut flip = false;

        while node != 0 {
            self.game_tree[node].w += if !flip { value } else { 1.0 - value };
            self.game_tree[node].n += 1;
            self.game_tree[node].virtual_loss -= 1.0;
            node = self.game_tree[node].parent;
            flip = !flip;
        }
    }

    pub fn visualize(&self, node: usize, node_num: usize) -> String {
        let mut dot = String::new();

        dot.push_str("digraph game_tree {\n");

        let mut nodes: std::vec::Vec<usize> = Vec::new();

        let mut counter: usize = 0;
        nodes.push(node);

        while counter < node_num && nodes.len() > 0 {
            let mut n_max: i32 = -1;
            let mut n_max_node = 0;
            let mut index = 0;

            for (i, n) in nodes.iter().enumerate() {
                if self.game_tree[*n].n as i32 > n_max  {
                    n_max = self.game_tree[*n].n as i32;
                    n_max_node = *n;
                    index = i;
                }
            }

            nodes.swap_remove(index);

            dot.push_str(&format!("  {} [label=\"N:{}\\nP:{:.3}\\nV:{:.3}\\nQ:{:.3}\"];\n", n_max_node,
                                                                                 self.game_tree[n_max_node].n,
                                                                                 self.game_tree[n_max_node].p,
                                                                                 self.game_tree[n_max_node].v,
                                                                                 if self.game_tree[n_max_node].n == 0 { 0.0 } else { self.game_tree[n_max_node].w / self.game_tree[n_max_node].n as f32 }).to_string());
            if self.game_tree[n_max_node].parent != 0 {
                dot.push_str(&format!("  {} -> {} [label=\"{}\"];\n", self.game_tree[n_max_node].parent, n_max_node, self.game_tree[n_max_node].m.sfen()).to_string());
            }

            counter += 1;
            for child in &self.game_tree[n_max_node].children {
                assert!(*child != 0);
                nodes.push(*child);
            }
        }

        dot.push_str("}");

        return dot;
    }

    pub fn debug(&self, node: usize) {
        for child in &self.game_tree[node].children {
            println!("{}, p:{:.3}, v:{:.3}, w:{:.3}, n:{:.3}, puct:{:.3}, vloss: {:.3}, parentn: {}", self.game_tree[*child].m.sfen(),
                                                            self.game_tree[*child].p,
                                                            self.game_tree[*child].v,
                                                            self.game_tree[*child].w,
                                                            self.game_tree[*child].n,
                                                            self.game_tree[*child].get_puct(self.game_tree[node].n as f32),
                                                            self.game_tree[*child].virtual_loss,
                                                            self.game_tree[node].n);
        }
    }
}

impl MCTS {
    pub fn select_puct_max_child(&self, node: usize) -> usize {
        let mut puct_max: f32 = -1.0;
        let mut puct_max_child: usize = 0;

        for child in &self.game_tree[node].children {
            let puct = self.game_tree[*child].get_puct(self.game_tree[node].n as f32 + self.game_tree[node].virtual_loss);

            if puct > puct_max {
                puct_max = puct;
                puct_max_child = *child;
            }
        }

        return puct_max_child;
    }

    pub fn select_n_max_child(&self, node: usize) -> usize {
        let mut n_max: u32 = 0;
        let mut n_max_child: usize = 0;

        for child in &self.game_tree[node].children {
            if self.game_tree[*child].n > n_max {
                n_max = self.game_tree[*child].n;
                n_max_child = *child;
            }
        }

        return n_max_child;
    }
}
