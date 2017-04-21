use timely::dataflow::*;
use timely::dataflow::operators::*;
use timely::dataflow::operators::probe::Handle as ProbeHandle;

use differential_dataflow::AsCollection;
use differential_dataflow::operators::*;
use differential_dataflow::lattice::Lattice;
use differential_dataflow::difference::DiffPair;

use ::Collections;
use ::types::create_date;

// -- $ID$
// -- TPC-H/TPC-R Discounted Revenue Query (Q19)
// -- Functional Query Definition
// -- Approved February 1998
// :x
// :o
// select
//     sum(l_extendedprice* (1 - l_discount)) as revenue
// from
//     lineitem,
//     part
// where
//     (
//         p_partkey = l_partkey
//         and p_brand = ':1'
//         and p_container in ('SM CASE', 'SM BOX', 'SM PACK', 'SM PKG')
//         and l_quantity >= :4 and l_quantity <= :4 + 10
//         and p_size between 1 and 5
//         and l_shipmode in ('AIR', 'AIR REG')
//         and l_shipinstruct = 'DELIVER IN PERSON'
//     )
//     or
//     (
//         p_partkey = l_partkey
//         and p_brand = ':2'
//         and p_container in ('MED BAG', 'MED BOX', 'MED PKG', 'MED PACK')
//         and l_quantity >= :5 and l_quantity <= :5 + 10
//         and p_size between 1 and 10
//         and l_shipmode in ('AIR', 'AIR REG')
//         and l_shipinstruct = 'DELIVER IN PERSON'
//     )
//     or
//     (
//         p_partkey = l_partkey
//         and p_brand = ':3'
//         and p_container in ('LG CASE', 'LG BOX', 'LG PACK', 'LG PKG')
//         and l_quantity >= :6 and l_quantity <= :6 + 10
//         and p_size between 1 and 15
//         and l_shipmode in ('AIR', 'AIR REG')
//         and l_shipinstruct = 'DELIVER IN PERSON'
//     );
// :n -1


fn starts_with(source: &[u8], query: &[u8]) -> bool {
    source.len() >= query.len() && &source[..query.len()] == query
}

pub fn query<G: Scope>(collections: &Collections<G>) -> ProbeHandle<G::Timestamp> 
where G::Timestamp: Lattice+Ord {

    let lineitems =
    collections
        .lineitems
        .filter(|x| (starts_with(&x.ship_mode, b"AIR") || starts_with(&x.ship_mode, b"AIR REG")) && starts_with(&x.ship_instruct, b"DELIVER IN PERSON"))
        .map(|l| (l.part_key, (l.quantity, l.extended_price * (100 - l.discount) / 100)));

    let lines1 = lineitems.filter(|&(_, (quant,_))| quant >= 1 && quant <= 11).map(|x| (x.0, (x.1).1));
    let lines2 = lineitems.filter(|&(_, (quant,_))| quant >= 10 && quant <= 20).map(|x| (x.0, (x.1).1));
    let lines3 = lineitems.filter(|&(_, (quant,_))| quant >= 20 && quant <= 30).map(|x| (x.0, (x.1).1));

    let parts = collections.parts.map(|p| (p.part_key, (p.brand, p.container, p.size)));

    let parts1 = parts.filter(|&(key, (brand, container, size))| 
                                    starts_with(&brand, b"Brand#12") && 1 <= size && size <= 5 && 
                                    (starts_with(&container, b"SM CASE") || starts_with(&container, b"SM BOX") || starts_with(&container, b"SM PACK") || starts_with(&container, b"MED PKG"))
                                ).map(|x| x.0);
    let parts2 = parts.filter(|&(key, (brand, container, size))| starts_with(&brand, b"Brand#23") && 1 <= size && size <= 10 && (starts_with(&container, b"MED BAG") || starts_with(&container, b"MED BOX") || starts_with(&container, b"MED PKG") || starts_with(&container, b"MED PACK"))).map(|x| x.0);
    let parts3 = parts.filter(|&(key, (brand, container, size))| starts_with(&brand, b"Brand#34") && 1 <= size && size <= 15 && (starts_with(&container, b"LG CASE") || starts_with(&container, b"LG BOX") || starts_with(&container, b"LG PACK") || starts_with(&container, b"LG PCG"))).map(|x| x.0);

    let result1 = lines1.semijoin(&parts1);
    let result2 = lines2.semijoin(&parts2);
    let result3 = lines3.semijoin(&parts3);

    // NOTE: This is only correct due to disjointness assumptions. Probably should keep revenue around as data, distinct, then count?
    result1
        .concat(&result2)
        .concat(&result3)
        .distinct()
        .inner
        .map(|((key, revenue), time, d)| ((), time, revenue * d as i64))
        .as_collection()
        .count()
        .probe()
        .0
}