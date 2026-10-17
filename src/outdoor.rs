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
pub struct Coverage {pub above:Option<f32>,pub floor:Option<f32>, pub supported:bool, below:bool, timer:f32}
impl Coverage {
    #[cfg(test)]
    pub fn update(&mut self, map:&DungeonMap, point:Point, dt:f32) {
        self.sample(map.triangles.iter(),point,dt);
    }
    pub fn update_window(&mut self,window:&Window,point:Point,dt:f32) {
        let cell=((point[0]/32.).floor() as i32,(point[2]/32.).floor() as i32);
        self.sample(window.cells.get(&cell).into_iter().flatten().map(|&i|&window.map.triangles[i]),point,dt);
    }
    fn sample<'a>(&mut self,triangles:impl Iterator<Item=&'a dungeon::Triangle>,point:Point,dt:f32) {
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
        self.above=above;self.floor=floor;self.supported=floor.is_some();
        let target=self.supported && above.is_some();
        if target!=self.below {
            self.timer+=dt.clamp(0.0,0.1);
            if self.timer>=0.35 {self.below=target;self.timer=0.0;}
        } else {self.timer=0.0;}
    }
    pub fn active(&self,mode:Mode)->bool {self.supported && match mode {Mode::Auto=>self.below,Mode::Local=>true,Mode::Surface=>false}}
}

pub struct Window {pub key:Key,pub name:String,pub map:DungeonMap,cells:HashMap<(i32,i32),Vec<usize>>}
impl Window {
    pub fn covers(&self,key:Key)->bool {self.key[0]==key[0] && self.key[3]==key[3]
        && self.key[1].abs_diff(key[1])<=1 && self.key[2].abs_diff(key[2])<=1}
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
    Ok(Window {key,name:catalog[&center].clone(),map,cells})
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
    fn lower_walkable_layer_wins_and_automatic_mode_has_hysteresis(){
        let map=DungeonMap {block:[60,34,42,0],triangles:vec![plane(240.),plane(410.)],boundaries:vec![],ramps:vec![]};
        let mut state=Coverage::default();
        for _ in 0..5 {state.update(&map,[0.,240.,0.],0.1);}
        assert!(state.active(Mode::Auto));assert_eq!(state.above,Some(170.));
        assert_eq!(map.floor_height([0.,240.1,0.]),240.);
        assert!(!state.active(Mode::Surface));
        state.update(&map,[0.,410.,0.],0.1);assert!(state.active(Mode::Auto));
        for _ in 0..4 {state.update(&map,[0.,410.,0.],0.1);}
        assert!(!state.active(Mode::Auto));assert!(state.active(Mode::Local));
        state.update(&map,[30.,240.,0.],0.1);assert!(!state.active(Mode::Local));
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
