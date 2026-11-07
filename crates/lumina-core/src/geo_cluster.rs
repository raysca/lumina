//! Grid clustering of geotagged items for the map. Pure: no catalog, no IO.

use crate::{GeoPoint, MediaId};
use std::collections::HashMap;

/// Items that fall in one grid cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cluster {
    /// Mean latitude of the members.
    pub lat: f32,
    /// Mean longitude of the members.
    pub lon: f32,
    pub count: usize,
    /// `[min_lat, min_lon, max_lat, max_lon]` of the members, the corner
    /// order of `within:lat1,lon1,lat2,lon2`. A cell never crosses the
    /// antimeridian, so `min_lon <= max_lon` always. These are f32 while the
    /// catalog stores f64, so an edge member can round outside them: build a
    /// search with [`Cluster::within_query`], which pads them outward by
    /// 1e-4° and clamps to ±90/±180, never from these directly.
    pub bounds: [f32; 4],
    /// The member with the lowest id, so the choice does not depend on
    /// input order.
    pub sample: MediaId,
}

/// How far [`Cluster::within_query`] widens the f32 bounds, in degrees
/// (~11 m). f32 rounding near ±180° is under 1e-5°.
const WITHIN_PAD_DEG: f64 = 1e-4;

impl Cluster {
    /// The `within:` search token that finds this cluster's members.
    pub fn within_query(&self) -> String {
        let [s, w, n, e] = self.bounds.map(f64::from);
        let lat = |v: f64| v.clamp(-90.0, 90.0);
        let lon = |v: f64| v.clamp(-180.0, 180.0);
        format!(
            "within:{},{},{},{}",
            lat(s - WITHIN_PAD_DEG),
            lon(w - WITHIN_PAD_DEG),
            lat(n + WITHIN_PAD_DEG),
            lon(e + WITHIN_PAD_DEG)
        )
    }
}

/// Buckets `points` into square cells of `cell_deg` degrees, in
/// O(n + k log k) for k clusters. Clusters come out sorted by cell, south to
/// north then west to east, so the same points give the same output in any
/// input order. A `cell_deg` that is not a positive number gives no
/// clusters.
pub fn cluster(points: &[GeoPoint], cell_deg: f32) -> Vec<Cluster> {
    if !(cell_deg > 0.0 && cell_deg.is_finite()) {
        return Vec::new();
    }
    struct Acc {
        lat_sum: f64,
        lon_sum: f64,
        count: usize,
        bounds: [f32; 4],
        sample: MediaId,
    }
    let cell = |deg: f32, offset: f32| ((deg + offset) / cell_deg).floor() as i64;
    let mut cells: HashMap<(i64, i64), Acc> = HashMap::new();
    for p in points {
        let acc = cells
            .entry((cell(p.lat, 90.0), cell(p.lon, 180.0)))
            .or_insert(Acc {
                lat_sum: 0.0,
                lon_sum: 0.0,
                count: 0,
                bounds: [p.lat, p.lon, p.lat, p.lon],
                sample: p.id,
            });
        acc.lat_sum += f64::from(p.lat);
        acc.lon_sum += f64::from(p.lon);
        acc.count += 1;
        let b = &mut acc.bounds;
        *b = [
            b[0].min(p.lat),
            b[1].min(p.lon),
            b[2].max(p.lat),
            b[3].max(p.lon),
        ];
        if p.id.0 < acc.sample.0 {
            acc.sample = p.id;
        }
    }
    let mut keyed: Vec<_> = cells.into_iter().collect();
    keyed.sort_unstable_by_key(|(key, _)| *key);
    keyed
        .into_iter()
        .map(|(_, acc)| {
            let n = acc.count as f64;
            let [s, w, n_lat, e] = acc.bounds;
            Cluster {
                // Clamped: the f32 mean can round a hair outside the bounds.
                lat: ((acc.lat_sum / n) as f32).clamp(s, n_lat),
                lon: ((acc.lon_sum / n) as f32).clamp(w, e),
                count: acc.count,
                bounds: acc.bounds,
                sample: acc.sample,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: u64, lat: f32, lon: f32) -> GeoPoint {
        GeoPoint {
            id: MediaId(id),
            lat,
            lon,
        }
    }

    #[test]
    fn clusters_merge_by_cell_and_split_on_zoom() {
        // Two Kyoto shots ~2 km apart and one in Paris.
        let points = [p(3, 35.01, 135.76), p(1, 35.03, 135.78), p(2, 48.85, 2.35)];

        let wide = cluster(&points, 10.0);
        assert_eq!(wide.len(), 2, "10° cells merge the Kyoto pair: {wide:?}");
        let kyoto = wide
            .iter()
            .find(|c| c.count == 2)
            .expect("the Kyoto pair must share one cluster");
        assert_eq!(kyoto.sample, MediaId(1), "sample is the lowest member id");
        assert!(
            (kyoto.lat - 35.02).abs() < 1e-4 && (kyoto.lon - 135.77).abs() < 1e-4,
            "the centre is the members' mean, got {kyoto:?}"
        );
        assert_eq!(
            wide.iter().map(|c| c.count).sum::<usize>(),
            3,
            "every point is counted once"
        );

        let close = cluster(&points, 0.01);
        assert_eq!(close.len(), 3, "0.01° cells split the pair: {close:?}");
        assert!(close.iter().all(|c| c.count == 1));

        let mut reversed = points;
        reversed.reverse();
        assert_eq!(
            cluster(&reversed, 10.0),
            wide,
            "output must not depend on input order"
        );
        assert_eq!(
            wide.iter().map(|c| c.count).collect::<Vec<_>>(),
            vec![2, 1],
            "clusters are ordered south to north (Kyoto, then Paris)"
        );

        assert!(
            cluster(&points, 0.0).is_empty(),
            "a zero cell gives nothing"
        );
        assert!(
            cluster(&points, f32::NAN).is_empty(),
            "a NaN cell gives nothing"
        );
        assert!(cluster(&[], 1.0).is_empty());
    }

    #[test]
    fn within_query_pads_f32_bounds_and_clamps() {
        // f64 coordinates whose f32 rounding lands inside the true extent.
        let truth: [(f64, f64); 3] = [
            (35.011_623_456_789, 135.768_123_456_789),
            (35.011_699_999_999, 135.768_199_999_999),
            (-33.868_812_345_678, 179.999_999_9),
        ];
        let points: Vec<GeoPoint> = truth
            .iter()
            .enumerate()
            .map(|(i, &(lat, lon))| p(i as u64, lat as f32, lon as f32))
            .collect();
        for c in cluster(&points, 1.0) {
            let token = c.within_query();
            let q = crate::StructuredQuery::parse(&token);
            let [s, w, n, e] = q
                .within
                .unwrap_or_else(|| panic!("{token:?} must parse as a geo filter"));
            assert!(w <= e, "a cluster box never wraps: {token}");
            let inside = truth
                .iter()
                .filter(|&&(lat, lon)| (s..=n).contains(&lat) && (w..=e).contains(&lon))
                .count();
            assert_eq!(
                inside, c.count,
                "{token} must hold every f64 member of its cluster"
            );
        }

        let pole = cluster(&[p(1, 90.0, 180.0), p(2, -90.0, -180.0)], 1.0);
        assert_eq!(pole.len(), 2);
        for c in pole {
            let token = c.within_query();
            assert!(
                crate::StructuredQuery::parse(&token).within.is_some(),
                "{token:?}: padding is clamped to ±90/±180, so it stays valid"
            );
        }
    }

    #[test]
    fn cluster_bounds_contain_members() {
        // A deterministic spread over the whole globe, edges included.
        let mut points: Vec<GeoPoint> = (0..500u64)
            .map(|i| {
                let lat = ((i * 37) % 181) as f32 - 90.0;
                let lon = ((i * 101) % 361) as f32 - 180.0;
                p(i, lat, lon)
            })
            .collect();
        points.push(p(900, 90.0, 180.0));
        points.push(p(901, -90.0, -180.0));

        for cell in [45.0, 5.0, 0.5] {
            let clusters = cluster(&points, cell);
            assert_eq!(
                clusters.iter().map(|c| c.count).sum::<usize>(),
                points.len(),
                "{cell}°: every point is in exactly one cluster"
            );
            for c in &clusters {
                let [s, w, n, e] = c.bounds;
                assert!(s <= n && w <= e, "{cell}°: bounds are ordered, got {c:?}");
                assert!(
                    (s..=n).contains(&c.lat) && (w..=e).contains(&c.lon),
                    "{cell}°: the centre lies inside the bounds, got {c:?}"
                );
                let members: Vec<_> = points
                    .iter()
                    .filter(|q| (s..=n).contains(&q.lat) && (w..=e).contains(&q.lon))
                    .collect();
                assert!(
                    members.len() >= c.count,
                    "{cell}°: the bounds hold all {} members, found {}",
                    c.count,
                    members.len()
                );
                assert!(
                    members.iter().any(|q| q.id == c.sample),
                    "{cell}°: the sample lies inside its cluster's bounds"
                );
                assert!(
                    members.iter().any(|q| q.lat == s)
                        && members.iter().any(|q| q.lat == n)
                        && members.iter().any(|q| q.lon == w)
                        && members.iter().any(|q| q.lon == e),
                    "{cell}°: the bounds are tight (each edge is a member), got {c:?}"
                );
            }
            // Each point is inside the bounds of the cluster of its own cell.
            for q in &points {
                assert!(
                    clusters.iter().any(|c| {
                        let [s, w, n, e] = c.bounds;
                        (s..=n).contains(&q.lat) && (w..=e).contains(&q.lon)
                    }),
                    "{cell}°: point {q:?} lies in no cluster's bounds"
                );
            }
        }
    }
}
