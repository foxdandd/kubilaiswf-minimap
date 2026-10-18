//! World-aligned cartography baked on one worker. The game only draws a texture disc.
use std::sync::{Arc,Mutex,atomic::{AtomicU64,Ordering},mpsc::{Sender,Receiver,channel}};
use crate::{dungeon::{self,Point},outdoor::Window};

pub const SIZE:usize=1024;
const PAPER:&[u8]=include_bytes!("../assets/cartography-paper.png");

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Key {pub window:u64,pub region:usize,x:i32,z:i32,height:i32,zoom:u8}
impl Key {
    pub fn new(window:&Window,region:usize,point:Point,floor:f32,zoom:usize)->Self {
        let range=[40.,80.,160.][zoom];
        Self {window:window.id,region,x:(point[0]/range).round() as i32,z:(point[2]/range).round() as i32,
            height:(floor/8.).round() as i32,zoom:zoom as u8}
    }
    pub fn span(self)->f32 {[160.,320.,640.][self.zoom as usize]}
    pub fn origin(self)->[f32;2] {
        let range=self.span()/4.;[self.x as f32*range-2.*range,self.z as f32*range-2.*range]
    }
    pub fn usable(self,window:&Window,region:usize,point:Point,floor:f32,range:f32)->bool {
        self.window==window.id && self.region==region && (floor-self.height as f32*8.).abs()<=12.
            && [point[0],point[2]].into_iter().zip(self.origin()).all(|(p,o)|p-range>=o && p+range<=o+self.span())
    }
}

pub struct Image {pub key:Key,pub rgba:Vec<u8>}
struct Request {key:Key,window:Arc<Window>,serial:u64}
pub struct Baker {requests:Sender<Request>,results:Mutex<Receiver<Image>>,serial:Arc<AtomicU64>,wanted:Option<Key>}
impl Baker {
    pub fn spawn()->Self {
        let (requests,rx)=channel::<Request>();let (tx,results)=channel();
        let serial=Arc::new(AtomicU64::new(0));let latest=serial.clone();
        std::thread::spawn(move || {
            let paper=Paper::load();
            while let Ok(mut request)=rx.recv() {
                for newer in rx.try_iter() {request=newer;}
                if let Some(image)=bake(&request.window,request.key,&paper,||latest.load(Ordering::Relaxed)!=request.serial) {
                    if tx.send(image).is_err() {break;}
                }
            }
        });
        Self {requests,results:Mutex::new(results),serial,wanted:None}
    }
    pub fn request(&mut self,window:&Arc<Window>,region:usize,point:Point,floor:f32,zoom:usize) {
        let key=Key::new(window,region,point,floor,zoom);
        // Keep a patch through grid/height rounding boundaries, including camera turns.
        if self.wanted.is_some_and(|k|k.zoom==zoom as u8 && k.usable(window,region,point,floor,[40.,80.,160.][zoom]+8.)) {return;}
        self.wanted=Some(key);let serial=self.serial.fetch_add(1,Ordering::Relaxed)+1;
        let _=self.requests.send(Request {window:Arc::clone(window),key,serial});
    }
    pub fn poll(&self)->Option<Image> {self.results.lock().unwrap().try_iter().last().filter(|i|Some(i.key)==self.wanted)}
    pub fn retry(&mut self) {self.wanted=None;}
    pub fn cancel(&mut self) {
        if self.wanted.take().is_some() {self.serial.fetch_add(1,Ordering::Relaxed);}
    }
}

struct Paper {rgba:Vec<u8>,width:usize,height:usize}
impl Paper {
    fn load()->Self {
        let mut decoder=png::Decoder::new(PAPER);decoder.set_transformations(png::Transformations::EXPAND|png::Transformations::ALPHA);
        let mut reader=decoder.read_info().expect("embedded paper");let mut rgba=vec![0;reader.output_buffer_size()];
        let info=reader.next_frame(&mut rgba).expect("embedded paper pixels");
        Self {rgba,width:info.width as usize,height:info.height as usize}
    }
    fn sample(&self,x:f32,z:f32)->[u8;3] {
        let x=(x/64.*self.width as f32).floor() as i32;let z=(z/64.*self.height as f32).floor() as i32;
        let at=(z.rem_euclid(self.height as i32) as usize*self.width+x.rem_euclid(self.width as i32) as usize)*4;
        std::array::from_fn(|i|self.rgba[at+i])
    }
}

struct Canvas {rgba:Vec<u8>,paper:Vec<[u8;3]>,origin:[f32;2],step:f32}
impl Canvas {
    fn new(key:Key,paper:&Paper)->Self {
        let origin=key.origin();let step=key.span()/SIZE as f32;
        let mut c=Self {rgba:vec![255;SIZE*SIZE*4],paper:Vec::with_capacity(SIZE*SIZE),origin,step};
        for y in 0..SIZE {for x in 0..SIZE {
            c.paper.push(paper.sample(origin[0]+(x as f32+0.5)*step,origin[1]+key.span()-(y as f32+0.5)*step));
            c.paint(x,y,[192,171,130]);
        }}c
    }
    fn paint(&mut self,x:usize,y:usize,tint:[u8;3]) {
        let at=y*SIZE+x;
        for i in 0..3 {self.rgba[at*4+i]=(self.paper[at][i] as u16*tint[i] as u16/255) as u8;}
    }
    fn project(&self,p:Point)->[f32;2] {[(p[0]-self.origin[0])/self.step,SIZE as f32-(p[2]-self.origin[1])/self.step]}
    fn polygon(&mut self,points:&[Point],tint:[u8;3]) {
        for i in 1..points.len().saturating_sub(1) {self.triangle([points[0],points[i],points[i+1]],tint);}
    }
    fn triangle(&mut self,world:[Point;3],tint:[u8;3]) {
        let [a,b,c]=world.map(|p|self.project(p));
        let edge=|a:[f32;2],b:[f32;2],p:[f32;2]|(b[0]-a[0])*(p[1]-a[1])-(b[1]-a[1])*(p[0]-a[0]);
        let area=edge(a,b,c);if area.abs()<1e-5 {return;}
        let low=[a,b,c].into_iter().fold([f32::INFINITY;2],|v,p|[v[0].min(p[0]),v[1].min(p[1])]);
        let high=[a,b,c].into_iter().fold([f32::NEG_INFINITY;2],|v,p|[v[0].max(p[0]),v[1].max(p[1])]);
        let sign=area.signum();
        for y in (low[1].floor().max(0.) as usize)..(high[1].ceil().clamp(0.,SIZE as f32) as usize) {
            for x in (low[0].floor().max(0.) as usize)..(high[0].ceil().clamp(0.,SIZE as f32) as usize) {
                let p=[x as f32+0.5,y as f32+0.5];
                if edge(a,b,p)*sign>= -1e-3 && edge(b,c,p)*sign>= -1e-3 && edge(c,a,p)*sign>= -1e-3 {self.paint(x,y,tint);}
            }
        }
    }
    fn line(&mut self,line:[Point;2],opacity:f32) {
        let [a,b]=line.map(|p|self.project(p));let steps=(a[0]-b[0]).hypot(a[1]-b[1]).ceil().min((SIZE*4) as f32) as usize;
        for i in 0..=steps {
            let t=i as f32/steps.max(1) as f32;let p=[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t];
            let x=p[0].round() as i32;let y=p[1].round() as i32;
            if x<0 || y<0 || x>=SIZE as i32 || y>=SIZE as i32 {continue;}
            let at=(y as usize*SIZE+x as usize)*4;
            for (c,ink) in [104.,80.,47.].into_iter().enumerate() {
                self.rgba[at+c]=(self.rgba[at+c] as f32*(1.-opacity)+ink*opacity) as u8;
            }
        }
    }
}

fn bake(window:&Window,key:Key,paper:&Paper,cancel:impl Fn()->bool)->Option<Image> {
    if cancel() {return None;}
    let center=key.origin().map(|v|v+key.span()/2.);let point=[center[0],key.height as f32*8.,center[1]];
    let range=key.span()/2.;let low=point[1]-6.;let high=point[1]+6.;let mut canvas=Canvas::new(key,paper);
    let nav=window.nav_candidates(point,range);let land=window.land_candidates(point,range);
    let physical=!window.map.land.is_empty();
    for (bottom,top,current,tint) in [(point[1]-40.,low,false,[113,99,77]),(high,point[1]+40.,false,[191,174,140]),(low,high,true,[246,236,208])] {
        if cancel() {return None;}
        let paint_nav=|canvas:&mut Canvas,water:bool| {
            for &i in &nav {
                if water && !window.nav_water[i] {continue;}
                let points=window.map.triangles[i].points;let primary=window.regions[i]==key.region;
                if !primary && (points.iter().all(|p|p[1]<bottom) || points.iter().all(|p|p[1]>top)) {continue;}
                let clipped=if primary {points.to_vec()}else{dungeon::clip_height(&points,bottom,top)};
                canvas.polygon(&clipped,if water {[153,164,145]}else{tint});
            }
        };
        if physical && current {paint_nav(&mut canvas,false);}
        let mut contours=Vec::new();
        let candidates=if physical {&land}else{&nav};
        for (n,&i) in candidates.iter().enumerate() {
            if n%1024==0 && cancel() {return None;}
            let points=if physical {window.map.land[i].points}else{window.map.triangles[i].points};
            if physical && window.map.land[i].water {continue;}
            let primary=if physical {window.land_regions[i]==Some(key.region)}else{window.regions[i]==key.region};
            if primary && !current {continue;}
            if !physical && !primary && window.region_areas[window.regions[i]]<12. {continue;}
            if !primary && (points.iter().all(|p|p[1]<bottom) || points.iter().all(|p|p[1]>top)) {continue;}
            let polygon=if primary {points.to_vec()}else{dungeon::clip_height(&points,bottom,top)};
            let relief=if current {crate::cartography::relief(points)}else{1.};
            canvas.polygon(&polygon,tint.map(|v|(v as f32*relief) as u8));
            if current {
                let min=if primary {points.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min)}else{bottom};
                let max=if primary {points.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max)}else{top};
                let mut height=(min/8.).ceil()*8.;let limit=max.min(height+128.);
                while height<limit {
                    if let Some(line)=crate::minimap::height_contour(points,height) {contours.push(line);}
                    height+=8.;
                }
            }
        }
        if physical && current {paint_nav(&mut canvas,true);}
        for line in contours {canvas.line(line,0.28);}
        if !physical && current {
            for (i,&line) in window.map.boundaries.iter().enumerate() {
                if window.boundary_regions[i]==Some(key.region) {canvas.line(line,0.45);}
            }
        }
    }
    (!cancel()).then_some(Image {key,rgba:canvas.rgba})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_geography_keeps_physical_ground_and_rejects_other_windows() {
        let p=[0.,240.,0.];let triangle=[[-30.,240.,-30.],[30.,240.,-30.],[0.,240.,30.]];
        let window=Window::from_map(dungeon::DungeonMap {block:[60,35,42,0],triangles:vec![dungeon::Triangle {points:triangle,section:0}],
            land:vec![crate::cartography::Land {points:triangle.map(|p|[p[0]*2.,p[1],p[2]*2.]),water:false}],boundaries:vec![],ramps:vec![],scenery:vec![]});
        let key=Key::new(&window,window.regions[0],p,240.,0);
        let paper=Paper::load();let image=bake(&window,key,&paper,||false).unwrap();
        assert_eq!(image.rgba.len(),SIZE*SIZE*4);
        let at=(SIZE/2*SIZE+SIZE/2)*4;assert!(image.rgba[at]>130);
        // (40,-40) is physical ground outside the smaller AI triangle.
        let pixel=SIZE*3/4;let world=-80.+(pixel as f32+0.5)*160./SIZE as f32;
        let expected=paper.sample(world,-world)[0] as u16*246/255;
        assert!((image.rgba[(pixel*SIZE+pixel)*4] as i32-expected as i32).abs()<=1);
        assert!(key.usable(&window,key.region,p,240.,40.));
        assert!(!Key {window:key.window+1,..key}.usable(&window,key.region,p,240.,40.));
        assert!(!key.usable(&window,key.region+1,p,240.,40.));
        assert!(!key.usable(&window,key.region,[200.,240.,0.],240.,40.));
        assert!(!key.usable(&window,key.region,p,280.,40.));
        assert!(bake(&window,key,&Paper::load(),||true).is_none());
    }
    #[test]
    fn latest_patch_wins_and_rounding_edges_do_not_restart_the_worker() {
        use std::time::{Duration,Instant};
        let window=Arc::new(Window::from_map(dungeon::DungeonMap {block:[60,35,42,0],triangles:vec![],land:vec![],boundaries:vec![],ramps:vec![],scenery:vec![]}));
        let mut baker=Baker::spawn();let p=[19.,240.,0.];
        baker.request(&window,0,p,240.,0);let serial=baker.serial.load(Ordering::Relaxed);
        baker.request(&window,0,[21.,240.,0.],240.,0);assert_eq!(baker.serial.load(Ordering::Relaxed),serial);
        baker.request(&window,0,p,240.,1);baker.request(&window,0,p,240.,2);
        let expected=baker.wanted.unwrap();let deadline=Instant::now()+Duration::from_secs(3);
        let image=loop {if let Some(image)=baker.poll() {break image;}
            assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));};
        assert_eq!(image.key,expected);assert_eq!(image.key.zoom,2);
        baker.request(&window,0,[1000.,240.,0.],240.,2);baker.cancel();assert!(baker.poll().is_none());
    }
}
