//! Height-aware outdoor navigation windows. Geometry is generated locally, never embedded.
use std::collections::HashMap;
use std::io;
use std::path::{Path,PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Sender,Receiver,channel};
use crate::dungeon::{self,DungeonMap,Point};
use crate::game::Snapshot;

pub type Key=[u8;4];
pub const HEIGHT_BAND:f32=6.0;
pub const CONTEXT_EXTENT:f32=40.0;

#[derive(Clone,Copy,Default,PartialEq,Eq)]
pub enum Mode { #[default] Auto, Local, Surface }
impl Mode {
    pub fn next(self)->Self {match self {Self::Auto=>Self::Local,Self::Local=>Self::Surface,Self::Surface=>Self::Auto}}
}

pub fn key(player:&Snapshot)->Option<Key> {
    if !matches!(player.block_id[0],60|61) || !player.position.iter().all(|v|v.is_finite())
        || !player.local_position[1].is_finite() {return None;}
    let grid=player.position.map(|v|((v+128.0)/256.0).floor());
    if grid.iter().any(|&v|!(0.0..=99.0).contains(&v)) {return None;}
    Some([player.block_id[0],grid[0] as u8,grid[1] as u8,player.block_id[3]/10*10])
}

pub fn position(player:&Snapshot)->Point {[player.position[0],player.local_position[1],player.position[1]]}

#[derive(Default)]
pub struct Coverage {
    pub above:Option<f32>, pub floor:Option<f32>, pub supported:bool,
    pub region:Option<usize>,
    below:bool, timer:f32, last:Option<Point>,anchor:Option<Point>,
}
impl Coverage {
    #[cfg(test)]
    pub fn update(&mut self, map:&DungeonMap, point:Point, dt:f32) {
        self.sample(map.triangles.iter(),point,dt,||None);
    }
    pub fn update_window(&mut self,window:&Window,point:Point,dt:f32) {
        let cell=((point[0]/32.).floor() as i32,(point[2]/32.).floor() as i32);
        let region=self.region;
        self.sample(window.cells.get(&cell).into_iter().flatten().map(|&i|&window.map.triangles[i]),point,dt,
            ||window.nearby_overhead(point,region));
        if self.supported {self.region=window.support_region(point,self.region);}
    }
    pub fn rebind(&mut self,window:&Window) {
        self.region=self.anchor.and_then(|point|window.support_region(point,None));
    }
    fn sample<'a>(&mut self,triangles:impl Iterator<Item=&'a dungeon::Triangle>,point:Point,dt:f32,
        nearby:impl FnOnce()->Option<f32>) {
        // A real warp/elevation jump must not carry a stale area latch with it.
        if self.last.is_some_and(|p|(p[0]-point[0]).hypot(p[2]-point[2])>80. || (p[1]-point[1]).abs()>32.) {
            *self=Self::default();
        }
        self.last=Some(point);
        let mut floor=None::<f32>;
        let mut above=None::<f32>;
        for triangle in triangles {
            if let Some(height)=dungeon::height_at(triangle.points,[point[0],point[2]]) {
                let delta=height-point[1];
                if (-8.0..=0.75).contains(&delta) && floor.is_none_or(|old| {
                    let old_below=old<=point[1]+0.35;let below=height<=point[1]+0.35;
                    (below && !old_below) || (below==old_below && delta.abs()<(old-point[1]).abs())
                }) {floor=Some(height);}
                if delta>if self.below {8.0} else {12.0} {above=Some(above.map_or(delta,|old|old.min(delta)));}
            }
        }
        // Once in an area, a small hole in the upper sheet should not mean sky.
        // Only use nearby overhead to retain an existing latch, never to enter it.
        if self.below && above.is_none() {above=nearby();}
        self.above=above;self.supported=floor.is_some();
        if let Some(height)=floor {self.floor=Some(height);self.anchor=Some([point[0],height,point[2]]);}
        // Missing AI support is unknown evidence, not an outdoor exit. Keep the
        // last floor and local layout while crossing holes or during a jump.
        if !self.supported {self.timer=0.;return;}
        let target=above.is_some();
        if target!=self.below {
            self.timer+=dt.clamp(0.0,0.1);
            let delay=if target {0.35} else {2.0};
            if self.timer>=delay {self.below=target;self.timer=0.0;}
        } else {self.timer=0.0;}
    }
    pub fn active(&self,mode:Mode)->bool {match mode {Mode::Auto=>self.below,Mode::Local=>self.floor.is_some(),Mode::Surface=>false}}
}

pub struct Window {
    pub key:Key,pub name:String,pub map:DungeonMap,
    pub regions:Vec<usize>,pub boundary_regions:Vec<Option<usize>>,
    cells:HashMap<(i32,i32),Vec<usize>>,
}
impl Window {
    pub fn covers(&self,key:Key)->bool {self.key[0]==key[0] && self.key[3]==key[3]
        && self.key[1].abs_diff(key[1])<=1 && self.key[2].abs_diff(key[2])<=1}

    /// A connected terrain sheet follows a hillside without turning it into floors.
    /// A nearby overlapping sheet cannot win on tiny navigation-height differences.
    pub fn support_region(&self,point:Point,preferred:Option<usize>)->Option<usize> {
        let cell=((point[0]/32.).floor() as i32,(point[2]/32.).floor() as i32);
        let mut best=None::<(f32,usize)>;let mut previous=None::<(f32,usize)>;
        for &i in self.cells.get(&cell).into_iter().flatten() {
            let Some(height)=dungeon::height_at(self.map.triangles[i].points,[point[0],point[2]]) else {continue;};
            if !(-8.0..=0.75).contains(&(height-point[1])) {continue;}
            let score=(height-point[1]).abs()+if height>point[1]+0.35 {1.0}else{0.0};
            let candidate=(score,self.regions[i]);
            if best.is_none_or(|old|score<old.0) {best=Some(candidate);}
            if Some(candidate.1)==preferred && previous.is_none_or(|old|score<old.0) {previous=Some(candidate);}
        }
        match (best,previous) {
            (Some(best),Some(previous)) if previous.0<=best.0+1.0=>Some(previous.1),
            (Some(best),_)=>Some(best.1),_=>None,
        }
    }

    fn nearby_overhead(&self,point:Point,region:Option<usize>)->Option<f32> {
        let mut above=None::<f32>;
        for x in ((point[0]-12.)/32.).floor() as i32..=((point[0]+12.)/32.).floor() as i32 {
            for z in ((point[2]-12.)/32.).floor() as i32..=((point[2]+12.)/32.).floor() as i32 {
                for &i in self.cells.get(&(x,z)).into_iter().flatten() {
                    if Some(self.regions[i])==region {continue;}
                    let triangle=self.map.triangles[i].points;
                    if triangle.iter().all(|p|p[1]<=point[1]+8.) {continue;}
                    let nearest=nearest_xz(triangle,[point[0],point[2]]);
                    if (nearest[0]-point[0]).hypot(nearest[1]-point[2])>12. {continue;}
                    let Some(height)=dungeon::height_at(triangle,nearest) else {continue;};
                    let delta=height-point[1];
                    if delta>8. {above=Some(above.map_or(delta,|old|old.min(delta)));}
                }
            }
        }
        above
    }

    #[cfg(test)]
    pub(crate) fn from_map(map:DungeonMap)->Self {index_window(map.block,String::new(),map).unwrap()}
}

fn nearest_xz(triangle:[Point;3],point:[f32;2])->[f32;2] {
    if dungeon::height_at(triangle,point).is_some() {return point;}
    let mut nearest=[triangle[0][0],triangle[0][2]];let mut distance=f32::INFINITY;
    for i in 0..3 {
        let a=triangle[i];let b=triangle[(i+1)%3];let d=[b[0]-a[0],b[2]-a[2]];
        let t=(((point[0]-a[0])*d[0]+(point[1]-a[2])*d[1])/(d[0]*d[0]+d[1]*d[1]).max(1e-8)).clamp(0.,1.);
        let p=[a[0]+d[0]*t,a[2]+d[1]*t];let length=(p[0]-point[0]).hypot(p[1]-point[1]);
        if length<distance {nearest=p;distance=length;}
    }
    nearest
}
pub struct Loader {requests:Sender<Key>, results:Mutex<Receiver<(Key,io::Result<Window>)>>}

impl Loader {
    pub fn spawn(directory:&Path,log:&Path)->io::Result<Self> {
        let mut catalog=HashMap::new();
        let text=std::fs::read_to_string(directory.join("maps.tsv"))?;
        let mut lines=text.lines();
        if lines.next()!=Some("map_id\tname") {return Err(invalid("invalid outdoor catalog header"));}
        for line in lines {
            let (id,name)=line.split_once('\t').ok_or_else(||invalid("invalid outdoor catalog"))?;
            let key=parse_id(id).ok_or_else(||invalid("invalid outdoor map ID"))?;
            if name.is_empty() || name.len()>512 || name.chars().any(char::is_control)
                || catalog.insert(key,name.to_owned()).is_some() || catalog.len()>2048 {return Err(invalid("invalid outdoor catalog entry"));}
        }
        if catalog.is_empty(){return Err(invalid("empty outdoor catalog"));}
        let directory=directory.to_owned();let log=log.to_owned();
        let (requests,rx)=channel();let (tx,results)=channel();
        std::thread::spawn(move || {
            while let Ok(mut key)=rx.recv() {
                for newer in rx.try_iter(){key=newer;}
                let result=load_window(&directory,&catalog,key);
                if let Err(error)=&result {let _=std::fs::write(&log,format!("Outdoor layout unavailable: {error}\n"));}
                if tx.send((key,result)).is_err(){break;}
            }
        });
        Ok(Self {requests,results:Mutex::new(results)})
    }
    pub fn request(&self,key:Key){let _=self.requests.send(key);}
    pub fn poll(&self)->Option<(Key,io::Result<Window>)>{self.results.lock().unwrap().try_iter().last()}
}

fn parse_id(id:&str)->Option<Key> {
    let bytes=id.as_bytes();
    if bytes.len()!=12 || bytes[0]!=b'm' || [3,6,9].iter().any(|&i|bytes[i]!=b'_'){return None;}
    let mut key=[0;4];
    for (i,offset) in [1,4,7,10].into_iter().enumerate(){
        if !bytes[offset..offset+2].iter().all(u8::is_ascii_digit){return None;}
        key[i]=(bytes[offset]-b'0')*10+bytes[offset+1]-b'0';
    }
    (matches!(key[0],60|61)&&matches!(key[3],0|10)).then_some(key)
}

fn file(directory:&Path,key:Key)->PathBuf {directory.join(format!("m{:02}_{:02}_{:02}_{:02}.nav",key[0],key[1],key[2],key[3]))}
fn invalid(message:&str)->io::Error {io::Error::new(io::ErrorKind::InvalidData,message)}

fn load_window(directory:&Path,catalog:&HashMap<Key,String>,key:Key)->io::Result<Window> {
    let select=|mut tile:Key| {if !catalog.contains_key(&tile){tile[3]=0;}catalog.contains_key(&tile).then_some(tile)};
    let center=select(key).ok_or_else(||io::Error::new(io::ErrorKind::NotFound,"no navigation for this tile"))?;
    let mut map=DungeonMap {block:key,triangles:vec![],boundaries:vec![],ramps:vec![]};
    for x in key[1].saturating_sub(1)..=key[1].saturating_add(1).min(99) {
        for z in key[2].saturating_sub(1)..=key[2].saturating_add(1).min(99) {
            let Some(tile)=select([key[0],x,z,key[3]]) else {continue;};
            let part=match DungeonMap::load_for_block(&file(directory,tile),tile) {
                Ok(part)=>part,Err(error) if tile!=center=>{let _=error;continue;},Err(error)=>return Err(error),
            };
            map.triangles.extend(part.triangles);map.boundaries.extend(part.boundaries);map.ramps.extend(part.ramps);
        }
    }
    index_window(key,catalog[&center].clone(),map)
}

fn vertex_key(point:Point)->[i32;3] {point.map(|v|(v*20.).round() as i32)}

fn index_window(key:Key,name:String,map:DungeonMap)->io::Result<Window> {
    // Built on the background loader, never rebuilt while drawing. Quantized 3D
    // vertices join tile seams without merging a bridge with the path below it.
    let mut parents:Vec<usize>=(0..map.triangles.len()).collect();
    fn root(parents:&mut [usize],mut i:usize)->usize {
        while parents[i]!=i {parents[i]=parents[parents[i]];i=parents[i];}i
    }
    let mut vertices=HashMap::new();
    for (i,triangle) in map.triangles.iter().enumerate() {
        for point in triangle.points {
            if let Some(&other)=vertices.get(&vertex_key(point)) {
                let a=root(&mut parents,i);let b=root(&mut parents,other);parents[a]=b;
            } else {vertices.insert(vertex_key(point),i);}
        }
    }
    let regions:Vec<_>=(0..parents.len()).map(|i|root(&mut parents,i)).collect();
    let boundary_regions=map.boundaries.iter().map(|line| {
        let a=*vertices.get(&vertex_key(line[0]))?;let b=*vertices.get(&vertex_key(line[1]))?;
        (regions[a]==regions[b]).then_some(regions[a])
    }).collect();
    let mut cells:HashMap<(i32,i32),Vec<usize>>=HashMap::new();let mut references=0;
    for (i,triangle) in map.triangles.iter().enumerate() {
        let bounds:[(i32,i32);2]=std::array::from_fn(|axis| {
            let a=if axis==0 {0}else{2};
            let low=triangle.points.iter().map(|p|p[a]).fold(f32::INFINITY,f32::min);
            let high=triangle.points.iter().map(|p|p[a]).fold(f32::NEG_INFINITY,f32::max);
            ((low/32.).floor() as i32,(high/32.).floor() as i32)
        });
        if bounds.iter().any(|(low,high)|high-low>32){return Err(invalid("outdoor polygon is too large"));}
        for x in bounds[0].0..=bounds[0].1 {for z in bounds[1].0..=bounds[1].1 {
            references+=1;if references>8_000_000{return Err(invalid("outdoor index is too large"));}
            cells.entry((x,z)).or_default().push(i);
        }}
    }
    Ok(Window {key,name,map,cells,regions,boundary_regions})
}

#[cfg(test)]
pub fn lower_sample(map:&DungeonMap)->Option<(Point,f32)> {
    // Use a broad walkable patch, rather than a tiny isolated terrain sliver.
    let area=|t:&dungeon::Triangle| {let [a,b,c]=t.points;((b[0]-a[0])*(c[2]-a[2])-(b[2]-a[2])*(c[0]-a[0])).abs()};
    let mut candidates:Vec<_>=map.triangles.iter().collect();
    candidates.sort_by(|a,b|area(b).total_cmp(&area(a)));
    candidates.into_iter().find_map(|t| {
        let p=std::array::from_fn(|i|t.points.iter().map(|p|p[i]).sum::<f32>()/3.);
        let mut state=Coverage::default();for _ in 0..5 {state.update(map,p,0.1);}
        state.active(Mode::Auto).then_some((p,state.above.unwrap_or(0.)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plane(y:f32)->dungeon::Triangle {dungeon::Triangle {points:[[-10.,y,-10.],[10.,y,-10.],[0.,y,10.]],section:0}}
    #[test]
    fn connected_hillsides_stay_one_region_and_bridges_stay_separate() {
        let low=plane(240.);let mut upper=plane(410.);
        for p in &mut upper.points {p[0]*=0.5;p[2]*=0.5;}
        let a=low.points[0];let b=low.points[1];
        let slope=dungeon::Triangle {points:[a,b,[0.,270.,-30.]],section:1};
        let map=DungeonMap {block:[60,34,42,0],triangles:vec![low,upper,slope],boundaries:vec![[a,b]],ramps:vec![]};
        let window=Window::from_map(map);
        assert_eq!(window.regions[0],window.regions[2]);
        assert_ne!(window.regions[0],window.regions[1]);
        assert_eq!(window.boundary_regions[0],Some(window.regions[0]));
        assert_eq!(window.support_region([0.,240.,0.],None),Some(window.regions[0]));
        assert_eq!(window.support_region([0.,410.,0.],Some(window.regions[0])),Some(window.regions[1]));
        let mut state=Coverage::default();for _ in 0..5 {state.update_window(&window,[0.,240.,0.],0.1);}
        // At x=4 there is support, but the exact upper triangle ends nearby.
        for _ in 0..30 {state.update_window(&window,[4.,240.,0.],0.1);assert!(state.supported);assert!(state.active(Mode::Auto));}
        state.rebind(&window);assert_eq!(state.region,Some(window.regions[0]));
        let mut triangles=vec![plane(10.)];
        triangles.extend(window.map.triangles.iter().map(|t|dungeon::Triangle {points:t.points,section:t.section}));
        let refreshed=Window::from_map(DungeonMap {block:window.key,triangles,boundaries:vec![],ramps:vec![]});
        let refreshed_region=refreshed.support_region([4.,240.,0.],None).unwrap();
        assert_ne!(window.regions[0],refreshed_region,"test must change region indices");
        state.rebind(&refreshed);
        state.update_window(&refreshed,[20.,240.,0.],0.1); // refresh followed by a support hole
        assert_eq!(state.region,Some(refreshed_region));assert!(state.active(Mode::Auto));
    }
    #[test]
    fn auto_keeps_the_local_map_through_navigation_holes() {
        let ground=DungeonMap {block:[60,34,42,0],triangles:vec![plane(240.),plane(410.)],boundaries:vec![],ramps:vec![]};
        let hole=DungeonMap {block:ground.block,triangles:vec![],boundaries:vec![],ramps:vec![]};
        let mut state=Coverage::default();
        for _ in 0..5 {state.update(&ground,[0.,240.,0.],0.1);}
        for _ in 0..15 {
            state.update(&hole,[0.,240.2,0.],0.1);
            assert!(state.active(Mode::Auto),"surface flash during a navigation hole");
            assert_eq!(state.floor,Some(240.));
        }
    }
    #[test]
    fn auto_requires_sustained_open_ground_before_returning_to_surface() {
        let covered=DungeonMap {block:[60,34,42,0],triangles:vec![plane(240.),plane(410.)],boundaries:vec![],ramps:vec![]};
        let open=DungeonMap {block:covered.block,triangles:vec![plane(240.)],boundaries:vec![],ramps:vec![]};
        let mut state=Coverage::default();
        for _ in 0..5 {state.update(&covered,[0.,240.,0.],0.1);}
        for _ in 0..8 {state.update(&open,[0.,240.,0.],0.1);assert!(state.active(Mode::Auto),"surface flash at sparse overhead data");}
        state.update(&covered,[0.,240.,0.],0.1);
        for _ in 0..25 {state.update(&open,[0.,240.,0.],0.1);}
        assert!(!state.active(Mode::Auto),"genuine open ground must still restore the surface");
    }
    #[test]
    fn lower_walkable_layer_wins_and_automatic_mode_has_hysteresis(){
        let map=DungeonMap {block:[60,34,42,0],triangles:vec![plane(240.),plane(410.)],boundaries:vec![],ramps:vec![]};
        let mut state=Coverage::default();
        for _ in 0..5 {state.update(&map,[0.,240.,0.],0.1);}
        assert!(state.active(Mode::Auto));assert_eq!(state.above,Some(170.));
        assert_eq!(map.floor_height([0.,240.1,0.]),240.);
        assert!(!state.active(Mode::Surface));
        state.update(&map,[0.,410.,0.],0.1);assert!(!state.active(Mode::Auto));
        for _ in 0..25 {state.update(&map,[0.,410.,0.],0.1);}
        assert!(!state.active(Mode::Auto));assert!(state.active(Mode::Local));
        state.update(&map,[200.,240.,0.],0.1);assert!(!state.active(Mode::Local));
        assert!(parse_id("m61_44_46_10").is_some());assert!(parse_id("m30_00_00_00").is_none());
        assert!(parse_id("../secret.nav").is_none());
    }
    #[test]
    #[ignore="Requires OUTDOOR_NAV_TEST_DIR from the installed game"]
    fn real_outdoor_tiles_decode_and_reveal_overlapping_walkable_levels(){
        let dir=std::env::var_os("OUTDOOR_NAV_TEST_DIR").expect("Set outdoor fixtures");let dir=Path::new(&dir);
        let mut checked=0;let mut examples=0;
        for entry in std::fs::read_dir(dir).unwrap(){
            let path=entry.unwrap().path();if path.extension().is_none_or(|v|v!="nav"){continue;}
            let map=DungeonMap::load(&path).unwrap();assert!(matches!(map.block[0],60|61));checked+=1;
            if [[60,34,42,0],[61,47,43,0],[61,45,46,0]].contains(&map.block){
                let (point,_)=lower_sample(&map).expect("Real lower path below an upper path");
                assert!((map.floor_height(point)-point[1]).abs()<0.1);examples+=1;
                if let Some(output)=std::env::var_os("OUTDOOR_SAMPLE_FILE") {
                    use std::io::Write;writeln!(std::fs::OpenOptions::new().create(true).append(true).open(output).unwrap(),"{:?} {:?}",map.block,point).unwrap();
                }
            }
        }
        assert!(checked>100);assert_eq!(examples,3);
    }
    #[test]
    #[ignore="Requires OUTDOOR_NAV_TEST_DIR from the installed game"]
    fn real_lower_terrain_walks_keep_the_region_and_mode_stable() {
        let dir=std::env::var_os("OUTDOOR_NAV_TEST_DIR").expect("Set outdoor fixtures");let dir=Path::new(&dir);
        let loader=Loader::spawn(dir,&dir.join("walking-test.log")).unwrap();
        let mut total=0;
        for key in [[60,34,42,0],[61,47,43,0],[61,45,46,0]] {
            let source=DungeonMap::load(&file(dir,key)).unwrap();let (start,_)=lower_sample(&source).unwrap();
            loader.request(key);
            let (_,result)=loader.results.lock().unwrap().recv_timeout(std::time::Duration::from_secs(15)).unwrap();
            let window=result.unwrap();let mut steps=0;
            for ray in 0..8 {
                let mut state=Coverage::default();for _ in 0..5 {state.update_window(&window,start,0.1);}
                assert!(state.active(Mode::Auto));let region=state.region.unwrap();
                let angle=std::f32::consts::TAU*ray as f32/8.;let mut last=start;
                for step in 1..=100 {
                    let mut point=[start[0]+angle.cos()*step as f32*0.6,last[1],start[2]+angle.sin()*step as f32*0.6];
                    let cell=((point[0]/32.).floor() as i32,(point[2]/32.).floor() as i32);
                    let height=window.cells.get(&cell).into_iter().flatten().filter(|&&i|window.regions[i]==region)
                        .filter_map(|&i|dungeon::height_at(window.map.triangles[i].points,[point[0],point[2]]))
                        .filter(|y|(y-last[1]).abs()<2.)
                        .min_by(|a,b|(a-last[1]).abs().total_cmp(&(b-last[1]).abs()));
                    let Some(height)=height else {break;};point[1]=height;
                    // Restrict this replay to the covered area; true exits are tested separately.
                    if window.nearby_overhead(point,Some(region)).is_none() {break;}
                    if step%17==0 {state.rebind(&window);}
                    state.update_window(&window,point,1./60.);
                    assert!(state.active(Mode::Auto),"surface flash at {key:?} {point:?}");
                    assert_eq!(state.region,Some(region),"terrain sheet changed at {key:?} {point:?}");
                    last=point;steps+=1;
                }
            }
            println!("Stable terrain replay {key:?}: {steps} walking samples");
            if key[0]==60 {assert!(steps>=200,"village trace too short");}
            assert!(steps>20);total+=steps;
        }
        assert!(total>=500);println!("Stable terrain replay total: {total} samples");
    }
    #[test]
    #[ignore="Requires OUTDOOR_NAV_TEST_DIR from the installed game"]
    fn real_outdoor_loader_keeps_worlds_and_event_variants_separate(){
        let dir=std::env::var_os("OUTDOOR_NAV_TEST_DIR").expect("Set outdoor fixtures");let dir=Path::new(&dir);
        let loader=Loader::spawn(dir,&dir.join("loader-test.log")).unwrap();
        for key in [[60,34,42,0],[61,47,43,0],[61,45,46,10]] {
            loader.request(key);
            let (returned,result)=loader.results.lock().unwrap().recv_timeout(std::time::Duration::from_secs(15)).unwrap();
            assert_eq!(returned,key);let window=result.unwrap();assert_eq!(window.key,key);
            assert!(window.covers(key));let mut other=key;other[0]=if key[0]==60 {61}else{60};assert!(!window.covers(other));
            assert!(window.map.triangles.len()>10_000);
            for triangle in window.map.triangles.iter().step_by((window.map.triangles.len()/32).max(1)) {
                let point=std::array::from_fn(|i|triangle.points.iter().map(|p|p[i]).sum::<f32>()/3.);
                let mut full=Coverage::default();let mut indexed=Coverage::default();
                full.update(&window.map,point,0.1);indexed.update_window(&window,point,0.1);
                assert_eq!(full.floor,indexed.floor);assert_eq!(full.above,indexed.above);
            }
        }
        loader.request([60,0,0,0]);
        assert!(loader.results.lock().unwrap().recv_timeout(std::time::Duration::from_secs(15)).unwrap().1.is_err());
        let _=std::fs::remove_file(dir.join("loader-test.log"));
    }
}
