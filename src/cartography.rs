//! Locally exported scenery; symbols are authored in code, never copied artwork.
use std::{io,path::Path};
use crate::dungeon::Point;

#[derive(Clone,Copy)]
pub struct Scenery {pub point:Point,pub radius:f32,pub yaw:f32,pub kind:u8}
pub struct Land {pub points:[Point;3],pub water:bool}
pub fn sort_land(faces:&mut [Land]) {
    // Paint higher surfaces over lower ones: water planes cannot cover a bank.
    let height=|face:&Land|face.points.iter().map(|p|p[1]).sum::<f32>();
    faces.sort_by(|a,b|height(a).total_cmp(&height(b)));
}

pub fn load_land(path:&Path,block:[u8;4])->io::Result<Vec<Land>> {
    match std::fs::metadata(path) {
        Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(vec![]),
        Err(e)=>return Err(e),Ok(meta) if meta.len()>20_000_016=>return Err(invalid()),_=>{},
    }
    decode_land(&std::fs::read(path)?,block)
}
fn decode_land(bytes:&[u8],block:[u8;4])->io::Result<Vec<Land>> {
    if bytes.len()<16 || &bytes[..8]!=b"ERLAND01" || bytes[8..12]!=block {return Err(invalid());}
    let count=u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if count>500_000 || bytes.len()!=16+count*40 {return Err(invalid());}
    bytes[16..].chunks_exact(40).map(|record| {
        let points:[Point;3]=std::array::from_fn(|i|std::array::from_fn(|j|
            f32::from_le_bytes(record[(i*3+j)*4..(i*3+j)*4+4].try_into().unwrap())));
        if points.iter().flatten().any(|v|!v.is_finite() || v.abs()>100_000.) || record[36]>1 {return Err(invalid());}
        Ok(Land {points,water:record[36]==1})
    }).collect()
}

pub fn load(path:&Path,block:[u8;4])->io::Result<Vec<Scenery>> {
    match std::fs::metadata(path) {
        Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(vec![]),
        Err(e)=>return Err(e),
        Ok(meta) if meta.len()>720_016=>return Err(invalid()),_=>{},
    }
    decode(&std::fs::read(path)?,block)
}
fn invalid()->io::Error {io::Error::new(io::ErrorKind::InvalidData,"invalid cartographic scenery")}
fn decode(bytes:&[u8],block:[u8;4])->io::Result<Vec<Scenery>> {
    if bytes.len()<16 || &bytes[..8]!=b"ERSCEN01" || bytes[8..12]!=block {return Err(invalid());}
    let count=u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if count>30_000 || bytes.len()!=16+count*24 {return Err(invalid());}
    bytes[16..].chunks_exact(24).map(|record| {
        let v:[f32;5]=std::array::from_fn(|i|f32::from_le_bytes(record[i*4..i*4+4].try_into().unwrap()));
        if !v.iter().all(|n|n.is_finite()) || v[..3].iter().any(|n|n.abs()>100_000.)
            || !(1.0..=7.0).contains(&v[3]) || record[20]>2 {return Err(invalid());}
        Ok(Scenery {point:[v[0],v[1],v[2]],radius:v[3],yaw:v[4],kind:record[20]})
    }).collect()
}

/// A fixed-world light direction models relief; quantization is deliberately subtle.
pub fn relief(points:[Point;3])->f32 {
    let [a,b,c]=points;let u=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];
    let v=[c[0]-a[0],c[1]-a[1],c[2]-a[2]];
    let mut n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
    if n[1]<0. {n=n.map(|n|-n);}
    let length=n.iter().map(|n|n*n).sum::<f32>().sqrt().max(0.001);
    let light=(n[0]*-0.5+n[1]*0.8+n[2]*0.3)/length;
    (0.936+0.08*light).clamp(0.85,1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_geometry_rejects_cross_map_data_and_paints_banks_over_water() {
        let mut bytes=b"ERLAND01".to_vec();bytes.extend([61,48,41,0]);bytes.extend(1u32.to_le_bytes());
        for n in [0f32,0.,0.,10.,0.,0.,0.,0.,10.] {bytes.extend(n.to_le_bytes());}bytes.extend([1,0,0,0]);
        assert_eq!(decode_land(&bytes,[61,48,41,0]).unwrap().len(),1);
        assert!(decode_land(&bytes,[60,48,41,0]).is_err());
        assert!(decode_land(&bytes[..bytes.len()-1],[61,48,41,0]).is_err());
        bytes[20..24].copy_from_slice(&f32::INFINITY.to_le_bytes());assert!(decode_land(&bytes,[61,48,41,0]).is_err());
        let mut faces=vec![Land {points:[[0.,1.,0.];3],water:false},Land {points:[[0.,0.,0.];3],water:true}];
        sort_land(&mut faces);assert!(faces[0].water);assert!(!faces[1].water);
    }
    #[test]
    fn scenery_rejects_wrong_map_truncation_and_invalid_numbers() {
        let mut bytes=b"ERSCEN01".to_vec();bytes.extend([61,48,41,0]);bytes.extend(1u32.to_le_bytes());
        for n in [12288f32,200.,10496.,3.,0.] {bytes.extend(n.to_le_bytes());}bytes.extend([0,0,0,0]);
        assert_eq!(decode(&bytes,[61,48,41,0]).unwrap().len(),1);
        assert!(decode(&bytes,[60,48,41,0]).is_err());
        assert!(decode(&bytes[..bytes.len()-1],[61,48,41,0]).is_err());
        bytes[16..20].copy_from_slice(&f32::NAN.to_le_bytes());assert!(decode(&bytes,[61,48,41,0]).is_err());
        assert_eq!(relief([[0.,0.,0.],[1.,0.,0.],[0.,0.,1.]]),1.);
    }
    #[test]
    #[ignore="Requires MINIMAP_SCENERY_TEST_ROOT from local MSB exports"]
    fn all_scenery_sidecars_decode_with_their_own_map_identity() {
        let root=std::env::var_os("MINIMAP_SCENERY_TEST_ROOT").expect("Set scenery fixtures");
        let mut files=0;let mut objects=0;let mut land=0;let mut water=0;
        for family in ["outdoor","dungeons"] {
            for entry in std::fs::read_dir(Path::new(&root).join(family)).unwrap() {
                let path=entry.unwrap().path();if path.extension().is_none_or(|e|e!="scenery") {continue;}
                let id=path.file_stem().unwrap().to_str().unwrap();
                let block=std::array::from_fn(|i|id[1+i*3..3+i*3].parse::<u8>().unwrap());
                let scenery=load(&path,block).unwrap();objects+=scenery.len();files+=1;
                let faces=load_land(&path.with_extension("land"),block).unwrap();
                water+=faces.iter().filter(|f|f.water).count();land+=faces.len();
            }
        }
        assert!(files>0);assert!(objects>0);
        println!("Decoded {files} scenery sidecars and {objects} classified MSB placements");
        println!("Decoded {land} optional physical terrain faces, including {water} water faces");
    }
}
