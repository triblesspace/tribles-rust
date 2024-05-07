//mod triblesetarchiveconstraint;

use bytes::Bytes;
use std::collections::HashSet;
use std::convert::TryInto;
use std::iter;
use triblesetarchiveconstraint::*;

use crate::query::TriblePattern;

use crate::{id_into_value, Handle};
use crate::{BlobParseError, Bloblike, Id, Value, Valuelike};
use core::panic;

use itertools::Itertools;

use sucds::bit_vectors::DArray;
use sucds::char_sequences::WaveletMatrix;
use sucds::mii_sequences::{EliasFano, EliasFanoBuilder};
use sucds::int_vectors::CompactVector;

use super::TribleSet;

#[derive(Debug, Clone)]
pub struct TribleSetArchive {
    pub domain: Vec<Value>,

    pub e_a: EliasFano,
    pub a_a: EliasFano,
    pub v_a: EliasFano,
    
    pub eav_c: WaveletMatrix<DArray>,
    pub vea_c: WaveletMatrix<DArray>,
    pub ave_c: WaveletMatrix<DArray>,
    pub vae_c: WaveletMatrix<DArray>,
    pub eva_c: WaveletMatrix<DArray>,
    pub aev_c: WaveletMatrix<DArray>,
}

impl From<&TribleSet> for TribleSetArchive {
    fn from(set: &TribleSet) -> Self {
        let triple_count = set.eav.len() as usize;
        assert!(triple_count > 0);

        // Domain
        let e_iter = set.eav.iter_prefix::<16>().map(|(e, _)| id_into_value(e));
        let a_iter = set.ave.iter_prefix::<16>().map(|(a, _)| id_into_value(a));
        let v_iter = set.vea.iter_prefix::<32>().map(|(v, _)| v);

        let domain: Vec<Value> = e_iter.merge(a_iter).merge(v_iter).dedup().collect();
        let alph_width = sucds::utils::needed_bits(domain.len()-1);
        
        let mut e_a = EliasFanoBuilder::new(domain.len(), triple_count).expect("|T| > 0");
        e_a.extend(set.eav.iter_prefix::<16>()
            .map(|(e, count)| (id_into_value(e), count as usize))
            .map(|(e, count)| (domain.binary_search(&e).expect("e in domain"), count))
            .flat_map(|(e, count)| iter::repeat(e).take(count)));
        let e_a = e_a.build();

        let mut a_a = EliasFanoBuilder::new(domain.len(), triple_count).expect("|T| > 0");
        a_a.extend(set.aev.iter_prefix::<16>()
            .map(|(a, count)| (id_into_value(a), count as usize))
            .map(|(a, count)| (domain.binary_search(&a).expect("e in domain"), count))
            .flat_map(|(a, count)| iter::repeat(a).take(count)));
        let a_a = a_a.build();

        let mut v_a = EliasFanoBuilder::new(domain.len(), triple_count).expect("|T| > 0");
        v_a.extend(set.vea.iter_prefix::<32>()
            .map(|(v, count)| (v, count as usize))
            .map(|(v, count)| (domain.binary_search(&v).expect("v in domain"), count))
            .flat_map(|(v, count)| iter::repeat(v).take(count)));
        let v_a = v_a.build();

        //eav
        let mut eav_c = CompactVector::with_capacity(triple_count, alph_width).expect("|D| > 2^32");
        eav_c.extend(set.eav.iter_prefix::<64>()
            .map(|(t, _)| t[32..64].try_into().unwrap())
            .map(|v| domain.binary_search(&v).expect("v in domain")));
        let eav_c = WaveletMatrix::<DArray>::new(eav_c).unwrap();

        //vea
        let mut vea_c = CompactVector::with_capacity(triple_count, alph_width).expect("|D| > 2^32");
        vea_c.extend(set.vea.iter_prefix::<64>()
            .map(|(t, _)| id_into_value(t[48..64].try_into().unwrap()))
            .map(|a| domain.binary_search(&a).expect("a in domain")));
        let vea_c = WaveletMatrix::<DArray>::new(vea_c).unwrap();

        //ave
        let mut ave_c = CompactVector::with_capacity(triple_count, alph_width).expect("|D| > 2^32");
        ave_c.extend(set.ave.iter_prefix::<64>()
            .map(|(t, _)| id_into_value(t[48..64].try_into().unwrap()))
            .map(|e| domain.binary_search(&e).expect("e in domain")));
        let ave_c = WaveletMatrix::<DArray>::new(ave_c).unwrap();

        //vae
        let mut vae_c = CompactVector::with_capacity(triple_count, alph_width).expect("|D| > 2^32");
        vae_c.extend(set.vae.iter_prefix::<64>()
            .map(|(t, _)| id_into_value(t[48..64].try_into().unwrap()))
            .map(|e| domain.binary_search(&e).expect("e in domain")));
        let vae_c = WaveletMatrix::<DArray>::new(vae_c).unwrap();

        //eva
        let mut eva_c = CompactVector::with_capacity(triple_count, alph_width).expect("|D| > 2^32");
        eva_c.extend(set.eva.iter_prefix::<64>()
            .map(|(t, _)| id_into_value(t[48..64].try_into().unwrap()))
            .map(|a| domain.binary_search(&a).expect("a in domain")));
        let eva_c = WaveletMatrix::<DArray>::new(eva_c).unwrap();

        //aev
        let mut aev_c = CompactVector::with_capacity(triple_count, alph_width).expect("|D| > 2^32");
        aev_c.extend(set.aev.iter_prefix::<64>()
            .map(|(t, _)| t[32..64].try_into().unwrap())
            .map(|v| domain.binary_search(&v).expect("v in domain")));
        let aev_c = WaveletMatrix::<DArray>::new(aev_c).unwrap();

        TribleSetArchive {
            domain,
            e_a,
            a_a,
            v_a,
            eav_c,
            vea_c,
            ave_c,
            vae_c,
            eva_c,
            aev_c,
        }
    }
}

/*
impl TriblePattern for TribleSetArchive {
    type PatternConstraint<'a, V>
     = TribleSetArchiveConstraint<'a, V>
     where V: Valuelike;

    fn pattern<'a, V>(
        &'a self,
        e: crate::query::Variable<Id>,
        a: crate::query::Variable<Id>,
        v: crate::query::Variable<V>,
    ) -> Self::PatternConstraint<'a, V>
    where
        V: Valuelike,
    {
        TribleSetArchiveConstraint::new(e, a, v, self)
    }
}
*/

impl<'a> Bloblike<'a> for TribleSetArchive {
    type Read = TribleSetArchive;

    fn read_blob(blob: &Bytes) -> Result<Self::Read, BlobParseError> {
        todo!()
    }

    fn into_blob(self) -> Bytes {
        todo!()
    }

    fn as_handle<H>(&self) -> Handle<H, Self>
    where
        H: digest::Digest + digest::OutputSizeUser<OutputSize = digest::consts::U32>,
    {
        todo!()
    }
}