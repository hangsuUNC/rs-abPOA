use crate::abpoa::{
    abpoa_add_graph_edge, abpoa_add_graph_node, abpoa_dump_pog, abpoa_init, abpoa_init_para,
    abpoa_msa, abpoa_para_t, abpoa_post_set_para, abpoa_t, free, strdup, ABPOA_SINK_NODE_ID,
    ABPOA_SRC_NODE_ID, FILE,
};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::ptr;

pub struct AbpoaAligner {
    ab: *mut abpoa_t,
    abpt: *mut abpoa_para_t,

    // NOTE: the following only work when adding the nodes manually!!!
    // TODO: fix this
    n_nodes: usize,
    nodes: Vec<Vec<i32>>,
    // this does not consider the initial and final edge, however this should not
    // cause any issue
    edges: Vec<(usize, usize)>,
}

pub struct AbpoaMSA {
    pub msa_length: usize,
    pub n_seqs: usize,
    // TODO: maybe this should be a map {seq_id : aln}?
    pub msa: Vec<String>,
}

impl AbpoaMSA {
    fn new() -> Self {
        AbpoaMSA {
            msa_length: 0,
            n_seqs: 0,
            msa: Vec::new(),
        }
    }

    fn new_from_alignment(msa: Vec<String>, n_seqs: usize, msa_length: usize) -> Self {
        AbpoaMSA {
            msa_length,
            n_seqs,
            msa,
        }
    }
}

pub struct AbpoaCons {
    pub cons_length: usize,
    pub cons: String,
}

impl AbpoaCons {
    fn new() -> Self {
        AbpoaCons {
            cons_length: 0,
            cons: String::new(),
        }
    }

    fn new_from_cons(cons: String) -> Self {
        AbpoaCons {
            cons_length: cons.len(),
            cons,
        }
    }
}

impl AbpoaAligner {
    pub unsafe fn new() -> Self {
        AbpoaAligner {
            ab: abpoa_init(),
            abpt: abpoa_init_para(),
            n_nodes: 0,
            nodes: vec![],
            edges: vec![],
        }
    }

    pub unsafe fn set_out_msa(&mut self, val: bool) {
        (*self.abpt).set_out_msa(val as u8);
    }

    pub unsafe fn set_out_cons(&mut self, val: bool) {
        (*self.abpt).set_out_cons(val as u8);
    }

    pub unsafe fn set_progressive_poa(&mut self, val: bool) {
        (*self.abpt).set_progressive_poa(val as u8);
    }

    pub unsafe fn set_w(&mut self, w: u8) {
        (*self.abpt).w = w as c_int;
    }

    pub unsafe fn set_k(&mut self, k: u8) {
        (*self.abpt).k = k as c_int;
    }

    pub unsafe fn set_min_w(&mut self, min_w: u8) {
        (*self.abpt).min_w = min_w as c_int;
    }

    pub unsafe fn set_post_para(&mut self) {
        abpoa_post_set_para(self.abpt);
    }

    pub unsafe fn reset_aligner(&mut self) {
        (*(*self.ab).abs).n_seq = 0;
    }

    // NOTE: Rust does not support static fields, using const is the closest thing to that
    // see: https://stackoverflow.com/a/48972982
    const NT4_TABLE: [u8; 256] = [
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, /*'-'*/
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 4, 1, 4, 4, 4, 2, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 4, 1, 4, 4, 4, 2, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    ];
    const ALN_ALPHABET: [char; 6] = ['A', 'C', 'G', 'T', 'N', '-'];
    const CONS_ALPHABET: [char; 5] = ['A', 'C', 'G', 'T', 'N'];

    pub unsafe fn align_seqs(&self, seqs: &[&str]) -> AbpoaMSA {
        // Get the number of input sequences
        let n_seqs: c_int = seqs.len() as c_int;

        // Create a Vec with the sequences' length
        let mut seq_lens: Vec<c_int> = seqs.iter().map(|s| s.len() as c_int).collect();

        // Generate bseqs
        let mut bseqs_val: Vec<Vec<u8>> = seqs
            .into_iter()
            .map(|s| {
                s.chars()
                    .map(|c| *(AbpoaAligner::NT4_TABLE).get(c as usize).unwrap())
                    .collect()
            })
            .collect();

        let mut bseqs: Vec<*mut u8> = bseqs_val.iter_mut().map(|s| s.as_mut_ptr()).collect();

        // Now perform the alignment
        let mut cons_seq: *mut *mut u8 = ptr::null_mut();
        let mut cons_c: *mut *mut c_int = ptr::null_mut();
        let mut cons_l: *mut c_int = ptr::null_mut();
        let mut cons_n: c_int = 0;
        let mut msa_seq: *mut *mut u8 = ptr::null_mut();
        let mut msa_l: c_int = 0;
        let out: *mut FILE = ptr::null_mut(); //stdout;

        abpoa_msa(
            self.ab,
            self.abpt,
            n_seqs,
            ptr::null_mut(),
            seq_lens.as_mut_ptr(),
            bseqs.as_mut_ptr(),
            out,
            &mut cons_seq,
            &mut cons_c,
            &mut cons_l,
            &mut cons_n,
            &mut msa_seq,
            &mut msa_l,
        );

        // Read the alignment's results
        let mut msa: Vec<String> = Vec::new();
        for i in 0..n_seqs {
            let mut curr_aln = String::with_capacity(msa_l as usize);
            let outer_pointer = *msa_seq.add((i) as usize);
            for j in 0..msa_l {
                let inner_pointer = *(outer_pointer.add(j as usize));
                curr_aln.push(
                    *AbpoaAligner::ALN_ALPHABET
                        .get(inner_pointer as usize)
                        .unwrap(),
                );
            }
            msa.push(curr_aln);
        }

        AbpoaMSA::new_from_alignment(msa, n_seqs as usize, msa_l as usize)
    }

    pub unsafe fn consensus_from_seqs(&self, seqs: &[&str]) -> AbpoaCons {
        // Get the number of input sequences
        let n_seqs: c_int = seqs.len() as c_int;

        // Create a Vec with the sequences' length
        let mut seq_lens: Vec<c_int> = seqs.iter().map(|s| s.len() as c_int).collect();

        // Generate bseqs
        let mut bseqs_val: Vec<Vec<u8>> = seqs
            .into_iter()
            .map(|s| {
                s.chars()
                    .map(|c| *(AbpoaAligner::NT4_TABLE).get(c as usize).unwrap())
                    .collect()
            })
            .collect();

        let mut bseqs: Vec<*mut u8> = bseqs_val.iter_mut().map(|s| s.as_mut_ptr()).collect();

        // Now perform the alignment
        let mut cons_seq: *mut *mut u8 = ptr::null_mut();
        let mut cons_c: *mut *mut c_int = ptr::null_mut();
        let mut cons_l: *mut c_int = ptr::null_mut();
        let mut cons_n: c_int = 0;
        let mut msa_seq: *mut *mut u8 = ptr::null_mut();
        let mut msa_l: c_int = 0;
        let out: *mut FILE = ptr::null_mut(); //stdout;

        abpoa_msa(
            self.ab,
            self.abpt,
            n_seqs,
            ptr::null_mut(),
            seq_lens.as_mut_ptr(),
            bseqs.as_mut_ptr(),
            out,
            &mut cons_seq,
            &mut cons_c,
            &mut cons_l,
            &mut cons_n,
            &mut msa_seq,
            &mut msa_l,
        );

        // Read the consensus
        let mut cons = String::with_capacity(*cons_l as usize);
        for i in 0..cons_n {
            let offset = *cons_l.add((i) as usize);
            for j in 0..offset {
                let outer_pointer = *cons_seq.add(i as usize);
                let inner_pointer = *(outer_pointer.add(j as usize));
                cons.push(
                    *AbpoaAligner::CONS_ALPHABET
                        .get(inner_pointer as usize)
                        .unwrap(),
                );
            }
        }

        AbpoaCons::new_from_cons(cons)
    }

    pub unsafe fn print_aln_to_dot(&mut self, path: &str) {
        // Build a C String to store path
        let c_str = CString::new(path).unwrap();

        (*self.abpt).out_pog = strdup(c_str.as_ptr() as *const c_char);
        abpoa_dump_pog(self.ab, self.abpt);
    }

    pub unsafe fn add_nodes_from_seq(&mut self, seq: &str) {
        let bseq: Vec<u8> = seq
            .chars()
            .map(|c| *(AbpoaAligner::NT4_TABLE).get(c as usize).unwrap())
            .collect();

        // First add the nodes to the graph
        // NOTE: in abpoa, each node has length 1 (i.e. a single nucleotide)
        let ids: Vec<i32> = bseq
            .into_iter()
            .map(|s| abpoa_add_graph_node((*self.ab).abg, s))
            .collect();

        //Then add the edges between said nodes
        ids.windows(2).map(|w| {
            abpoa_add_graph_edge(
                (*self.ab).abg,
                *w.get(0).unwrap(),
                *w.get(1).unwrap(),
                0,
                1,
                0,
                0,
                0,
            )
        });

        // Update wrapper data
        self.n_nodes += seq.len();
        self.nodes.push(ids);
    }

    pub unsafe fn add_edge(&mut self, from_node_id: i32, to_node_id: i32) {
        abpoa_add_graph_edge((*self.ab).abg, from_node_id, to_node_id, 0, 1, 0, 0, 0);
    }

    pub unsafe fn add_nodes_edges(&mut self, nodes: &Vec<&str>, edges: &Vec<(usize, usize)>) {
        // Add nodes
        nodes.iter().for_each(|n| self.add_nodes_from_seq(n));

        // Add edges between nodes
        edges.iter().for_each(|e| {
            self.edges.push((e.0, e.1));
            let last_of_start_node = self.nodes.get(e.0).unwrap().last().unwrap();
            let first_of_end_node = self.nodes.get(e.1).unwrap().first().unwrap();
            self.add_edge(*last_of_start_node, *first_of_end_node);
        });

        if self.n_nodes > 0 {
            // Add initial edge
            abpoa_add_graph_edge(
                (*self.ab).abg,
                ABPOA_SRC_NODE_ID as i32,
                *self.nodes.first().unwrap().first().unwrap(),
                0,
                1,
                0,
                0,
                0,
            );

            // Add last edge
            abpoa_add_graph_edge(
                (*self.ab).abg,
                *self.nodes.last().unwrap().last().unwrap(),
                ABPOA_SINK_NODE_ID as i32,
                0,
                1,
                0,
                0,
                0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aln() {
        unsafe {
            let mut aligner = AbpoaAligner::new();

            aligner.set_out_msa(true);
            aligner.set_out_cons(true);
            aligner.set_w(6);
            aligner.set_k(9);
            aligner.set_min_w(10);
            aligner.set_progressive_poa(true);

            aligner.set_post_para();

            let seqs: Vec<&str> = [
                "CGTCAATCTATCGAAGCATACGCGGGCAGAGCCGAAGACCTCGGCAATCCA",
                "CCACGTCAATCTATCGAAGCATACGCGGCAGCCGAACTCGACCTCGGCAATCAC",
                "CGTCAATCTATCGAAGCATACGCGGCAGAGCCCGGAAGACCTCGGCAATCAC",
                "CGTCAATGCTAGTCGAAGCAGCTGCGGCAGAGCCGAAGACCTCGGCAATCAC",
                "CGTCAATCTATCGAAGCATTCTACGCGGCAGAGCCGACCTCGGCAATCAC",
                "CGTCAATCTAGAAGCATACGCGGCAAGAGCCGAAGACCTCGGCCAATCAC",
                "CGTCAATCTATCGGTAAAGCATACGCTCTGTAGCCGAAGACCTCGGCAATCAC",
                "CGTCAATCTATCTTCAAGCATACGCGGCAGAGCCGAAGACCTCGGCAATC",
                "CGTCAATGGATCGAGTACGCGGCAGAGCCGAAGACCTCGGCAATCAC",
                "CGTCAATCTAATCGAAGCATACGCGGCAGAGCCGTCTACCTCGGCAATCACGT",
            ]
            .to_vec();

            let aln = aligner.align_seqs(&*seqs);

            //aligner.print_aln_to_dot("example.png");

            aligner.reset_aligner();

            //println!("MSA: {:#?}", aln.msa);
            assert_eq!(aln.n_seqs, seqs.len());
            assert_eq!(aln.msa_length, 75);
        }
    }

    #[test]
    fn test_cons() {
        unsafe {
            let mut aligner = AbpoaAligner::new();

            aligner.set_out_msa(true);
            aligner.set_out_cons(true);
            aligner.set_w(6);
            aligner.set_k(9);
            aligner.set_min_w(10);
            aligner.set_progressive_poa(true);

            aligner.set_post_para();

            let seqs: Vec<&str> = [
                "CGTCAATCTATCGAAGCATACGCGGGCAGAGCCGAAGACCTCGGCAATCCA",
                "CCACGTCAATCTATCGAAGCATACGCGGCAGCCGAACTCGACCTCGGCAATCAC",
                "CGTCAATCTATCGAAGCATACGCGGCAGAGCCCGGAAGACCTCGGCAATCAC",
                "CGTCAATGCTAGTCGAAGCAGCTGCGGCAGAGCCGAAGACCTCGGCAATCAC",
                "CGTCAATCTATCGAAGCATTCTACGCGGCAGAGCCGACCTCGGCAATCAC",
                "CGTCAATCTAGAAGCATACGCGGCAAGAGCCGAAGACCTCGGCCAATCAC",
                "CGTCAATCTATCGGTAAAGCATACGCTCTGTAGCCGAAGACCTCGGCAATCAC",
                "CGTCAATCTATCTTCAAGCATACGCGGCAGAGCCGAAGACCTCGGCAATC",
                "CGTCAATGGATCGAGTACGCGGCAGAGCCGAAGACCTCGGCAATCAC",
                "CGTCAATCTAATCGAAGCATACGCGGCAGAGCCGTCTACCTCGGCAATCACGT",
            ]
            .to_vec();

            let cons = aligner.consensus_from_seqs(&*seqs);

            //aligner.print_aln_to_dot("example.png");

            aligner.reset_aligner();

            println!("Cons: {:#?}", cons.cons);
            //assert_eq!(aln.n_seqs, seqs.len());
            //assert_eq!(aln.msa_length, 75);
        }
    }

    #[test]
    fn test_add_nodes() {
        unsafe {
            let mut aligner = AbpoaAligner::new();
            aligner.add_nodes_from_seq("ACGT");
            assert_eq!(aligner.nodes.first().unwrap().len(), 4);
            assert_eq!(aligner.n_nodes, 4);
        }
    }

    #[test]
    fn test_add_nodes_and_edges() {
        unsafe {
            let mut aligner = AbpoaAligner::new();
            aligner.add_nodes_edges(&vec!["ACG", "GCT", "TAT"], &vec![(0, 1), (0, 2)]);
            assert_eq!(aligner.nodes.len(), 3);
            assert_eq!(aligner.n_nodes, 9);
            assert_eq!(aligner.edges.len(), 2);
        }
    }
}
