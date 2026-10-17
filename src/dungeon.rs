//! Offline-generated navigation surfaces. No game-derived geometry is embedded in the DLL.
use std::io;
use std::path::Path;
use std::collections::HashMap;

#[cfg(test)]
pub const PILOT_BLOCK: [u8; 4] = [30, 0, 0, 0];
pub const ZOOM: [f32; 3] = [15.0, 30.0, 60.0];
pub type Point = [f32; 3];

pub struct Triangle {
    pub points: [Point; 3],
    // Kept for source-model provenance; display colours follow height rather than model IDs.
    #[allow(dead_code)]
    pub section: u32,
}

pub struct DungeonMap {
    pub block: [u8; 4],
    pub triangles: Vec<Triangle>,
    pub boundaries: Vec<[Point; 2]>,
    pub ramps: Vec<Ramp>,
}

#[derive(Clone, Copy, Debug)]
pub struct Ramp { pub bottom: Point, pub top: Point }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View { Layered, Current, All }

impl View {
    pub fn next(self) -> Self {
        match self { Self::Layered=>Self::Current, Self::Current=>Self::All, Self::All=>Self::Layered }
    }
}

#[derive(Default)]
pub struct FloorTracker { height: Option<f32>, last: Option<Point> }

impl FloorTracker {
    pub fn update(&mut self, map: &DungeonMap, player: Point, dt: f32) {
        let target=map.floor_height(player);
        self.update_height(target,player,dt);
    }
    pub fn update_height(&mut self,target:f32,player:Point,dt:f32) {
        let reset=self.last.is_none_or(|last| (last[0]-player[0]).hypot(last[2]-player[2])>12.0)
            || self.height.is_none_or(|height| (target-height).abs()>3.0);
        self.height=Some(if reset {target} else {
            let height=self.height.unwrap();
            height+(target-height)*(1.0-(-14.0*dt.clamp(0.0,0.1)).exp())
        });
        self.last=Some(player);
    }
    pub fn height(&self) -> Option<f32> {self.height}
    pub fn reset(&mut self) {self.height=None;self.last=None;}
}

impl DungeonMap {
    pub fn load(path: &Path) -> io::Result<Self> {
        let size = std::fs::metadata(path)?.len();
        if size > 32 * 1024 * 1024 {
            return Err(invalid("dungeon data exceeds 32 MiB"));
        }
        Self::decode(&std::fs::read(path)?)
    }

    pub fn load_for_block(path: &Path, block: [u8;4]) -> io::Result<Self> {
        let map=Self::load(path)?;
        if map.block!=block {return Err(invalid("dungeon file belongs to another map"));}
        Ok(map)
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        if data.len() < 20 || (&data[..8]!=b"ERDNG001" && &data[..8]!=b"ERWLD001") {
            return Err(invalid("invalid dungeon data header"));
        }
        let block: [u8; 4] = data[8..12].try_into().unwrap();
        let valid=if &data[..8]==b"ERWLD001" {
            matches!(block[0],60|61) && matches!(block[3],0|10)
        } else {(10..60).contains(&block[0])};
        if !valid || block.iter().any(|&v|v>99) {
            return Err(invalid("invalid non-overworld dungeon block"));
        }
        let n = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let b = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
        if n == 0 || n > 200_000 || b > 200_000 || data.len() != 20 + n * 40 + b * 24 {
            return Err(invalid("invalid dungeon record counts or length"));
        }
        let mut at = 20;
        let mut triangles = Vec::with_capacity(n);
        for _ in 0..n {
            let points = [read_point(data, &mut at)?, read_point(data, &mut at)?, read_point(data, &mut at)?];
            let section = u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
            at += 4;
            triangles.push(Triangle { points, section });
        }
        let mut boundaries = Vec::with_capacity(b);
        for _ in 0..b {
            boundaries.push([read_point(data, &mut at)?, read_point(data, &mut at)?]);
        }
        let ramps=find_ramps(&triangles);
        Ok(Self { block, triangles, boundaries, ramps })
    }

    /// Find the closest AI surface directly above/below the player's XZ, within 4 m.
    /// This follows slopes while avoiding a higher floor when the player jumps.
    pub fn floor_height(&self, player: Point) -> f32 {
        let heights: Vec<_>=self.triangles.iter().filter_map(|triangle| {
            height_at(triangle.points, [player[0], player[2]])
        }).filter(|height| (height - player[1]).abs() <= 4.0).collect();
        // Prefer support below the feet over a closer ceiling/upper storey while jumping.
        heights.iter().copied().filter(|height| *height<=player[1]+0.35)
            .min_by(|a,b| (a-player[1]).abs().total_cmp(&(b-player[1]).abs()))
            .or_else(|| heights.into_iter().min_by(|a,b| (a-player[1]).abs().total_cmp(&(b-player[1]).abs())))
            .unwrap_or(player[1])
    }
}

fn find_ramps(triangles: &[Triangle]) -> Vec<Ramp> {
    let mut candidates=Vec::new();
    for triangle in triangles {
        let min=triangle.points.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min);
        let max=triangle.points.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max);
        let span=(0..3).map(|i| {
            let a=triangle.points[i];let b=triangle.points[(i+1)%3];(a[0]-b[0]).hypot(a[2]-b[2])
        }).fold(0.0,f32::max);
        if max-min>0.15 && (max-min)/span.max(0.01)>0.12 {candidates.push(triangle);}
    }
    let mut parents:Vec<_>=(0..candidates.len()).collect();
    fn root(parents:&mut [usize], mut i:usize)->usize {
        while parents[i]!=i {parents[i]=parents[parents[i]];i=parents[i];} i
    }
    let mut vertices=HashMap::new();
    for (i,triangle) in candidates.iter().enumerate() {
        for point in triangle.points {
            let key=point.map(|v|(v*20.0).round() as i32);
            if let Some(&other)=vertices.get(&key) {
                let a=root(&mut parents,i);let b=root(&mut parents,other);parents[a]=b;
            } else {vertices.insert(key,i);}
        }
    }
    let mut groups:HashMap<usize,Vec<Point>>=HashMap::new();
    for (i,triangle) in candidates.iter().enumerate() {
        groups.entry(root(&mut parents,i)).or_default().extend(triangle.points);
    }
    let mut ramps=Vec::new();
    for points in groups.values() {
        let low=points.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min);
        let high=points.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max);
        if high-low<1.2 {continue;}
        let average=|height:f32| {
            let selected:Vec<_>=points.iter().filter(|p|(p[1]-height).abs()<0.2).collect();
            std::array::from_fn(|axis| selected.iter().map(|p|p[axis]).sum::<f32>()/selected.len() as f32)
        };
        let bottom:Point=average(low);let top:Point=average(high);
        let span=(top[0]-bottom[0]).hypot(top[2]-bottom[2]);
        if (1.0..=30.0).contains(&span) {ramps.push(Ramp {bottom,top});}
    }
    ramps.sort_by(|a,b| (b.top[1]-b.bottom[1]).total_cmp(&(a.top[1]-a.bottom[1]))
        .then_with(||a.bottom[0].total_cmp(&b.bottom[0])).then_with(||a.bottom[2].total_cmp(&b.bottom[2])));
    let mut distinct:Vec<Ramp>=Vec::new();
    for ramp in ramps {
        if distinct.iter().all(|other| {
            let a=[(ramp.bottom[0]+ramp.top[0])*0.5,(ramp.bottom[2]+ramp.top[2])*0.5];
            let b=[(other.bottom[0]+other.top[0])*0.5,(other.bottom[2]+other.top[2])*0.5];
            (a[0]-b[0]).hypot(a[1]-b[1])>3.0 || (ramp.bottom[1]-other.bottom[1]).abs()>1.5
        }) {distinct.push(ramp);}
    }
    distinct
}

fn read_point(data: &[u8], at: &mut usize) -> io::Result<Point> {
    let mut point = [0.0; 3];
    for value in &mut point {
        *value = f32::from_le_bytes(data[*at..*at + 4].try_into().unwrap());
        *at += 4;
        if !value.is_finite() || value.abs() > 1_000_000.0 {
            return Err(invalid("non-finite or unreasonable dungeon coordinate"));
        }
    }
    Ok(point)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn height_at([a, b, c]: [Point; 3], [x, z]: [f32; 2]) -> Option<f32> {
    let denominator = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if denominator.abs() < 1e-7 { return None; }
    let u = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / denominator;
    let v = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / denominator;
    let w = 1.0 - u - v;
    (u >= -1e-4 && v >= -1e-4 && w >= -1e-4).then_some(u * a[1] + v * b[1] + w * c[1])
}

/// Sutherland-Hodgman clipping to a Y slab, retaining the part of stairs in the slab.
pub fn clip_height(points: &[Point], low: f32, high: f32) -> Vec<Point> {
    let mut result = points.to_vec();
    for (plane, sign) in [(low, 1.0), (high, -1.0)] {
        let input = std::mem::take(&mut result);
        if input.is_empty() { break; }
        let mut previous = *input.last().unwrap();
        let mut pd = (previous[1] - plane) * sign;
        for current in input {
            let cd = (current[1] - plane) * sign;
            if (pd >= 0.0) != (cd >= 0.0) {
                let t = pd / (pd - cd);
                result.push(std::array::from_fn(|i| previous[i] + (current[i] - previous[i]) * t));
            }
            if cd >= 0.0 { result.push(current); }
            previous = current;
            pd = cd;
        }
    }
    result
}

/// Clip to an inscribed 32-sided disc; map polygons cannot spill beyond the frame.
pub fn clip_disc(points: &[[f32; 2]], radius: f32) -> Vec<[f32; 2]> {
    let limit = radius * (std::f32::consts::PI / 32.0).cos();
    // Dense outdoor meshes mostly lie wholly inside or outside the disc. Avoid
    // 32 allocating clipping passes for each of those tiny terrain triangles.
    if points.iter().all(|p|p[0]*p[0]+p[1]*p[1]<=limit*limit) {return points.to_vec();}
    let nearest:[f32;2]=std::array::from_fn(|axis| {
        let low=points.iter().map(|p|p[axis]).fold(f32::INFINITY,f32::min);
        let high=points.iter().map(|p|p[axis]).fold(f32::NEG_INFINITY,f32::max);
        if low>0.0 {low} else if high<0.0 {high} else {0.0}
    });
    if nearest[0]*nearest[0]+nearest[1]*nearest[1]>radius*radius {return vec![];}
    let mut result = points.to_vec();
    for side in 0..32 {
        let (sin, cos) = (std::f32::consts::TAU * (side as f32 + 0.5) / 32.0).sin_cos();
        let input = std::mem::take(&mut result);
        if input.is_empty() { break; }
        let mut previous = *input.last().unwrap();
        let mut pd = limit - previous[0] * cos - previous[1] * sin;
        for current in input {
            let cd = limit - current[0] * cos - current[1] * sin;
            if (pd >= 0.0) != (cd >= 0.0) {
                let t = pd / (pd - cd);
                result.push(std::array::from_fn(|i| previous[i] + (current[i] - previous[i]) * t));
            }
            if cd >= 0.0 { result.push(current); }
            previous = current;
            pd = cd;
        }
    }
    result
}

pub fn clip_segment_height(mut line: [Point; 2], low: f32, high: f32) -> Option<[Point; 2]> {
    for (plane, sign) in [(low, 1.0), (high, -1.0)] {
        let a = (line[0][1] - plane) * sign;
        let b = (line[1][1] - plane) * sign;
        if a < 0.0 && b < 0.0 { return None; }
        if (a >= 0.0) != (b >= 0.0) {
            let t = a / (a - b);
            let point = std::array::from_fn(|i| line[0][i] + (line[1][i] - line[0][i]) * t);
            line[if a < 0.0 { 0 } else { 1 }] = point;
        }
    }
    Some(line)
}

pub fn clip_segment_disc([a, b]: [[f32; 2]; 2], radius: f32) -> Option<[[f32; 2]; 2]> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let aa = d[0] * d[0] + d[1] * d[1];
    if aa < 1e-8 { return None; }
    let bb = 2.0 * (a[0] * d[0] + a[1] * d[1]);
    let cc = a[0] * a[0] + a[1] * a[1] - radius * radius;
    let discriminant = bb * bb - 4.0 * aa * cc;
    if discriminant < 0.0 { return None; }
    let t0 = ((-bb - discriminant.sqrt()) / (2.0 * aa)).max(0.0);
    let t1 = ((-bb + discriminant.sqrt()) / (2.0 * aa)).min(1.0);
    if t0 > t1 { return None; }
    Some([std::array::from_fn(|i| a[i] + d[i] * t0), std::array::from_fn(|i| a[i] + d[i] * t1)])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn triangle(y: f32) -> Triangle {
        Triangle { points: [[0.0, y, 0.0], [10.0, y, 0.0], [0.0, y, 10.0]], section: 0 }
    }
    #[test]
    fn stacked_floors_follow_player_height() {
        let map = DungeonMap { block: PILOT_BLOCK, triangles: vec![triangle(0.0), triangle(5.0)], boundaries: vec![], ramps:vec![] };
        assert_eq!(map.floor_height([1.0, 0.2, 1.0]), 0.0);
        assert_eq!(map.floor_height([1.0, 5.2, 1.0]), 5.0);
        assert_eq!(map.floor_height([20.0, 7.0, 20.0]), 7.0);
    }
    #[test]
    fn slopes_have_interpolated_heights() {
        assert_eq!(height_at([[0., 0., 0.], [10., 10., 0.], [0., 0., 10.]], [2., 2.]), Some(2.0));
        assert_eq!(height_at(triangle(0.).points, [9., 9.]), None);
    }
    #[test]
    fn jumping_does_not_select_a_closer_upper_floor() {
        let map=DungeonMap {block:PILOT_BLOCK,triangles:vec![triangle(0.),triangle(5.)],boundaries:vec![],ramps:vec![]};
        assert_eq!(map.floor_height([1.,3.,1.]),0.);
        assert_eq!(map.floor_height([1.,5.05,1.]),5.);
        let mut tracker=FloorTracker::default();
        tracker.update(&map,[1.,0.,1.],1./60.);
        tracker.update(&map,[1.,3.,1.],1./60.);
        assert_eq!(tracker.height(),Some(0.));
        tracker.update(&map,[1.,5.05,1.],1./60.);
        assert_eq!(tracker.height(),Some(5.));
        let close=DungeonMap {block:PILOT_BLOCK,triangles:vec![triangle(0.),triangle(0.3)],boundaries:vec![],ramps:vec![]};
        assert_eq!(close.floor_height([1.,0.05,1.]),0.);
    }
    #[test]
    fn floor_changes_follow_slopes_smoothly_and_reset_on_warps() {
        let slope=Triangle {points:[[0.,0.,0.],[10.,5.,0.],[0.,0.,10.]],section:0};
        let map=DungeonMap {block:PILOT_BLOCK,triangles:vec![slope],boundaries:vec![],ramps:vec![]};
        let mut tracker=FloorTracker::default();
        tracker.update(&map,[0.,0.,1.],1./60.);
        tracker.update(&map,[4.,2.,1.],1./60.);
        assert!((0.0..2.0).contains(&tracker.height().unwrap()));
        for _ in 0..45 {tracker.update(&map,[4.,2.,1.],1./60.);}
        assert!((tracker.height().unwrap()-2.0).abs()<0.01);
        tracker.update(&map,[100.,15.,100.],1./60.);
        assert_eq!(tracker.height(),Some(15.));
        tracker.reset();assert_eq!(tracker.height(),None);
    }
    #[test]
    fn ramp_markers_group_slopes_and_skip_flat_rooms() {
        let a=Triangle {points:[[0.,0.,0.],[4.,4.,0.],[0.,0.,2.]],section:0};
        let b=Triangle {points:[[0.,0.,2.],[4.,4.,0.],[4.,4.,2.]],section:0};
        let ramps=find_ramps(&[a,b,triangle(0.)]);
        assert_eq!(ramps.len(),1);
        assert_eq!(ramps[0].bottom[1],0.);
        assert_eq!(ramps[0].top[1],4.);
    }
    #[test]
    fn stairs_are_cut_at_height_boundaries() {
        let clipped = clip_height(&[[0., 0., 0.], [10., 10., 0.], [0., 0., 10.]], 3., 6.);
        assert!(clipped.len() >= 3);
        assert!(clipped.iter().all(|p| (2.999..=6.001).contains(&p[1])));
        assert!(clip_height(&triangle(8.).points, 0., 2.).is_empty());
    }
    #[test]
    fn polygons_and_lines_stay_inside_disc() {
        let points = clip_disc(&[[-20., -20.], [20., -20.], [20., 20.], [-20., 20.]], 10.);
        assert!(points.len() >= 16 && points.iter().all(|p| p[0]*p[0]+p[1]*p[1] <= 100.001));
        let [a,b] = clip_segment_disc([[-20.,0.],[20.,0.]],10.).unwrap();
        assert_eq!([a,b],[[-10.,0.],[10.,0.]]);
        assert!(clip_segment_disc([[20.,20.],[30.,30.]],10.).is_none());
    }
    #[test]
    fn boundary_segments_follow_height_slab() {
        assert_eq!(clip_segment_height([[0.,0.,0.],[10.,10.,0.]],3.,6.),Some([[3.,3.,0.],[6.,6.,0.]]));
        assert!(clip_segment_height([[0.,8.,0.],[1.,8.,0.]],0.,2.).is_none());
    }
    #[test]
    fn decoder_accepts_other_dungeons_and_rejects_bad_data() {
        let mut data = b"ERDNG001".to_vec(); data.extend(PILOT_BLOCK);
        data.extend(1u32.to_le_bytes()); data.extend(0u32.to_le_bytes());
        for p in triangle(0.).points { for x in p { data.extend(x.to_le_bytes()); } }
        data.extend(0u32.to_le_bytes());
        assert!(DungeonMap::decode(&data).is_ok());
        assert!(DungeonMap::decode(&data[..data.len()-1]).is_err());
        data[8]=31; assert!(DungeonMap::decode(&data).is_ok());
        data[8]=60; assert!(DungeonMap::decode(&data).is_err());
        data[8]=30;
        data[20..24].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(DungeonMap::decode(&data).is_err());
    }
    #[test]
    #[ignore = "Requires TOMBSWARD_NAV_TEST_FILE generated from the local game"]
    fn real_export_tracks_every_triangle_centroid() {
        let path = std::env::var_os("TOMBSWARD_NAV_TEST_FILE").expect("Set the local fixture path");
        let map = DungeonMap::load(Path::new(&path)).unwrap();
        assert_eq!(map.triangles.len(), 3324);
        assert_eq!(map.boundaries.len(), 1859);
        for triangle in &map.triangles {
            let center: Point = std::array::from_fn(|axis| triangle.points.iter().map(|p| p[axis]).sum::<f32>() / 3.0);
            let floor = map.floor_height([center[0], center[1]+0.05, center[2]]);
            assert!((floor-center[1]).abs() < 0.15, "Surface mismatch at {center:?}");
        }
    }
}
