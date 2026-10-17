use std::collections::VecDeque;
use std::f32::consts::{PI, TAU};
use std::path::{Path, PathBuf};

use hudhook::imgui::{Context, DrawListMut, FontSource, ImColor32, TextureId, Ui};
use hudhook::{ImguiRenderLoop, RenderContext};

use crate::config::Config;
use crate::dungeon::{self, DungeonMap, FloorTracker, View};
use crate::dungeon_loader::DungeonLoader;
use crate::game::{self, PointKind, Snapshot};
use crate::maptiles::{self, Loader, MapId, TILE, WINDOW};
use crate::outdoor;

/// Meters from the center to the rim at each zoom level.
const ZOOM_RANGES: [f32; 3] = [40.0, 80.0, 160.0];
const TRAIL_SPACING: f32 = 1.5;
const TRAIL_LENGTH: usize = 800;
const TELEPORT_DISTANCE: f32 = 150.0;
const FONT_SIZE: f32 = 13.0;
/// Radius of the map disc at 1080p.
const RADIUS: f32 = 96.0;
/// Map pixels (meters) the window must extend past the player: the widest zoom plus slack.
const WINDOW_MARGIN: f32 = ZOOM_RANGES[2] + 20.0;
/// Segments of the textured map disc.
const DISC_SEGMENTS: usize = 72;

const FRAME_PNG: &[u8] = include_bytes!("../assets/frame.png");
const ARROW_PNG: &[u8] = include_bytes!("../assets/arrow.png");
const PAPER_PNG: &[u8] = include_bytes!("../assets/cartography-paper.png");
const PAPER_PERIOD: f32 = 64.0;
const BEACON_PNG: &[u8] = include_bytes!("../assets/marker.png");
/// Map radius as a fraction of frame.png's width: just past the widest point of its hole
/// (measured 0.387 to 0.406) so the map always reaches under the frame.
const FRAME_HOLE: f32 = 0.408;
/// Point of arrow.png it turns around: the middle of the arrowhead, not of the canvas.
const ARROW_PIVOT: [f32; 2] = [0.5, 603.0 / 1254.0];

const PALE_GOLD: [u8; 3] = [214, 196, 150];
const BRIGHT_GOLD: [u8; 3] = [255, 222, 150];
const BACKGROUND: [u8; 3] = [24, 20, 15];
const BLACK: [u8; 3] = [0, 0, 0];
const LEVEL_FILL: [u8;3] = [173,155,117];
const LEVEL_EDGE: [u8;3] = [233,209,158];
const UPPER_FILL: [u8;3] = [36,48,53];
const UPPER_EDGE: [u8;3] = [108,156,173];
const LOWER_FILL: [u8;3] = [39,32,24];
const LOWER_EDGE: [u8;3] = [131,108,76];

/// Filled neighbouring triangles must share opaque edges. Keep AA on for real outlines.
struct SolidMeshFill { raw: *mut hudhook::imgui::sys::ImDrawList, flags:i32 }
impl SolidMeshFill {
    fn foreground() -> Self {
        // SAFETY: used only while this render thread owns an active foreground draw list.
        unsafe {
            let raw=hudhook::imgui::sys::igGetForegroundDrawList();
            let flags=(*raw).Flags;
            (*raw).Flags &= !(hudhook::imgui::sys::ImDrawListFlags_AntiAliasedFill as i32);
            Self {raw,flags}
        }
    }
}
impl Drop for SolidMeshFill {
    fn drop(&mut self) {
        // SAFETY: the guard is scoped inside the same active frame and draw-list borrow.
        unsafe {(*self.raw).Flags=self.flags;}
    }
}

#[cfg(test)]
mod dungeon_render_tests {
    use super::*;
    static IMGUI_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn paper_sample(uv:[f32;2])->[f32;4] {
        static PAPER:std::sync::OnceLock<(Vec<u8>,usize,usize)>=std::sync::OnceLock::new();
        let (bytes,width,height)=PAPER.get_or_init(|| {
            let mut decoder=png::Decoder::new(PAPER_PNG);
            decoder.set_transformations(png::Transformations::EXPAND|png::Transformations::ALPHA);
            let mut reader=decoder.read_info().unwrap();let mut bytes=vec![0;reader.output_buffer_size()];
            let info=reader.next_frame(&mut bytes).unwrap();assert_eq!(info.color_type,png::ColorType::Rgba);
            (bytes,info.width as usize,info.height as usize)
        });
        let x=uv[0]* *width as f32-0.5;let y=uv[1]* *height as f32-0.5;
        let fx=x-x.floor();let fy=y-y.floor();
        std::array::from_fn(|c| {
            let pixel=|dx:i32,dy:i32| {
                let ix=(x.floor() as i32+dx).rem_euclid(*width as i32) as usize;
                let iy=(y.floor() as i32+dy).rem_euclid(*height as i32) as usize;
                bytes[(iy*width+ix)*4+c] as f32/255.0
            };
            (pixel(0,0)*(1.-fx)+pixel(1,0)*fx)*(1.-fy)+(pixel(0,1)*(1.-fx)+pixel(1,1)*fx)*fy
        })
    }

    fn sample(data: &hudhook::imgui::DrawData, point: [f32;2]) -> [f32;3] {
        let mut color = [24.0,20.0,15.0];
        for list in data.draw_lists() {
            for command in list.commands() {
            let hudhook::imgui::DrawCmd::Elements {count,cmd_params}=command else {continue;};
            for indices in list.idx_buffer()[cmd_params.idx_offset..cmd_params.idx_offset+count].chunks_exact(3) {
                let v: [_;3] = std::array::from_fn(|i| list.vtx_buffer()[indices[i] as usize+cmd_params.vtx_offset]);
                let [a,b,c] = v.map(|v| v.pos);
                let d=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1]);
                if d.abs()<1e-6 {continue;}
                let u=((b[1]-c[1])*(point[0]-c[0])+(c[0]-b[0])*(point[1]-c[1]))/d;
                let w=((c[1]-a[1])*(point[0]-c[0])+(a[0]-c[0])*(point[1]-c[1]))/d;
                let weights=[u,w,1.0-u-w];
                if weights.iter().any(|x| *x < -1e-5) {continue;}
                let uv=std::array::from_fn(|axis|(0..3).map(|i|weights[i]*v[i].uv[axis]).sum());
                let texture=if cmd_params.texture_id.id()==1 {paper_sample(uv)} else {[1.;4]};
                let alpha=(0..3).map(|i| weights[i]*v[i].col[3] as f32/255.0).sum::<f32>()*texture[3];
                for channel in 0..3 {
                    let source=(0..3).map(|i| weights[i]*v[i].col[channel] as f32).sum::<f32>();
                    color[channel]=source*texture[channel]*alpha+color[channel]*(1.0-alpha);
                }
            }
            }
        }
        color
    }

    fn test_config() -> Config {
        Config { trail:false, rotate:true, compass:true, hide_in_boss_fights:false,
            hide_in_caves:true, dungeon_mode:true, outdoor_layers:true, parchment_maps:true, dungeon_height_band:2.0,
            dungeon_floor_key:None, size:1.0, margin:24.0, toggle_key:None, zoom_key:None }
    }

    fn player_at(local_position: [f32;3]) -> Snapshot {
        Snapshot { block_id:dungeon::PILOT_BLOCK, local_position,
            position:[local_position[0],local_position[2]], space:30, map:None,
            facing:[0.,1.], camera_right:[1.,0.], camera_forward:[0.,1.], fade:0.,
            hud_visible:true, boss_fight:false, in_cave:true, graces:vec![], beacons:vec![], points:vec![] }
    }

    #[test]
    fn paper_stays_fixed_in_world_space_across_movement_rotation_and_zoom() {
        let mut minimap=Minimap::new(Path::new("missing-test-game"),test_config(),None);
        let world=[8709.25,10777.75];let expected=world.map(|v|v/PAPER_PERIOD);
        for angle in [0.0_f32,0.7,2.4] {
            minimap.camera_right=[angle.cos(),angle.sin()];
            minimap.camera_forward=[-angle.sin(),angle.cos()];
            for position in [[8700.,237.,10700.],[8710.,245.,10780.]] {
                for scale in [0.6,2.4,6.4] {
                    let screen=minimap.to_screen([world[0]-position[0],world[1]-position[2]]).map(|v|v*scale);
                    let uv=minimap.paper_uv(screen,position,scale);
                    assert!((0..2).all(|i|(uv[i]-expected[i]).abs()<0.00003));
                }
            }
        }
    }

    #[test]
    fn contours_follow_slopes_and_join_at_shared_triangle_edges() {
        let a=[[0.,0.,0.],[10.,4.,0.],[10.,4.,10.]];
        let b=[[0.,0.,0.],[10.,4.,10.],[0.,0.,10.]];
        let first=height_contour(a,2.).unwrap();let second=height_contour(b,2.).unwrap();
        assert!(first.contains(&[5.,2.,5.]) && second.contains(&[5.,2.,5.]));
        assert!(first.iter().chain(second.iter()).all(|p|p[1]==2.));
        assert!(height_contour([[0.,0.,0.],[10.,0.,0.],[0.,0.,10.]],0.).is_none());
        assert!(height_contour(a,5.).is_none());
        assert!(height_contour(a,4.).is_none());
    }

    #[test]
    fn outdoor_tiles_use_centered_cells_and_keep_event_variants() {
        let mut player=player_at([0.,240.,0.]);player.block_id=[60,17,21,1];
        player.position=[34.*256.-100.,42.*256.-100.];
        assert_eq!(outdoor::key(&player),Some([60,34,42,0]));
        player.position[0]=34.*256.+128.;assert_eq!(outdoor::key(&player),Some([60,35,42,0]));
        player.block_id=[61,17,21,11];assert_eq!(outdoor::key(&player),Some([61,35,42,10]));
        player.position[0]=f32::NAN;assert!(outdoor::key(&player).is_none());
        player.position=[0.,0.];player.local_position[1]=f32::NAN;assert!(outdoor::key(&player).is_none());
        player.local_position[1]=240.;
        player.position=[0.,0.];player.block_id=[30,0,0,0];assert!(outdoor::key(&player).is_none());
    }

    fn dump_scene(path:&Path,data:&hudhook::imgui::DrawData) {
        use std::io::Write;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file=std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        write!(file,"{{\"lists\":[").unwrap();
        for (list_index,list) in data.draw_lists().enumerate() {
            if list_index>0 {write!(file,",").unwrap();}
            write!(file,"{{\"vertices\":[").unwrap();
            for (i,v) in list.vtx_buffer().iter().enumerate() {
                if i>0 {write!(file,",").unwrap();}
                write!(file,"[{},{},{},{},{},{},{},{}]",v.pos[0],v.pos[1],v.uv[0],v.uv[1],v.col[0],v.col[1],v.col[2],v.col[3]).unwrap();
            }
            let mut indices=Vec::new();
            let mut commands=Vec::new();
            for command in list.commands() {
                if let hudhook::imgui::DrawCmd::Elements {count,cmd_params}=command {
                    commands.push(format!("{{\"first\":{},\"count\":{count},\"texture\":{}}}",indices.len(),cmd_params.texture_id.id()));
                    indices.extend(list.idx_buffer()[cmd_params.idx_offset..cmd_params.idx_offset+count].iter()
                        .map(|&v|v as usize+cmd_params.vtx_offset));
                }
            }
            write!(file,"],\"indices\":{indices:?},\"commands\":[{}]}}",commands.join(",")).unwrap();
        }
        write!(file,"]}}").unwrap();
    }

    #[test]
    fn adjoining_floor_triangles_have_no_dark_diagonal_seam() {
        let _lock=IMGUI_TEST_LOCK.lock().unwrap();
        let map=DungeonMap { block:dungeon::PILOT_BLOCK, triangles:vec![
            dungeon::Triangle {points:[[0.,0.,0.],[10.,0.,0.],[10.,0.,10.]],section:0},
            dungeon::Triangle {points:[[0.,0.,0.],[10.,0.,10.],[0.,0.,10.]],section:0}],boundaries:vec![],ramps:vec![] };
        let mut minimap=Minimap::new(Path::new("missing-test-game-directory"),test_config(),Some(map));
        minimap.alpha=1.0;
        minimap.paper=Some(TextureId::new(1));
        let mut context=Context::create();
        context.set_ini_filename(None);
        context.io_mut().display_size=[800.,600.];
        context.io_mut().delta_time=1./60.;
        context.io_mut().backend_flags.insert(hudhook::imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        context.fonts().build_rgba32_texture();
        {
            let ui=context.frame();
            let draw=ui.get_foreground_draw_list();
            let flags=unsafe {(*hudhook::imgui::sys::igGetForegroundDrawList()).Flags};
            minimap.draw_dungeon(ui,&draw,&player_at([5.,0.,5.]),[400.,300.],96.,1.);
            assert_eq!(unsafe {(*hudhook::imgui::sys::igGetForegroundDrawList()).Flags},flags);
        }
        let data=context.render();
        // Compare each pixel with its own material sample: neighbouring paper grain varies.
        for point in [[400.02,299.97],[405.0,300.0]] {
            let actual=sample(data,point);
            let uv=minimap.paper_uv([point[0]-400.,point[1]-300.],[5.,0.,5.],96./dungeon::ZOOM[minimap.zoom]);
            let texture=paper_sample(uv);
            let expected:[f32;3]=std::array::from_fn(|i|texture[i]*[246.,236.,208.][i]);
            assert!((0..3).all(|i|(actual[i]-expected[i]).abs()<2.),"point={point:?}, actual={actual:?}, expected={expected:?}");
        }
    }

    #[test]
    #[ignore = "Requires TOMBSWARD_NAV_TEST_FILE generated from the local game"]
    fn real_dungeon_generates_finite_imgui_draw_data_inside_frame() {
        let _lock=IMGUI_TEST_LOCK.lock().unwrap();
        let path = std::env::var_os("TOMBSWARD_NAV_TEST_FILE").expect("Set local fixture path");
        let map = DungeonMap::load(Path::new(&path)).unwrap();
        let mut poses=Vec::new();
        for (name,model,target_y) in [("Entrance",100,56.5),("Stairs",200,49.0),("Lower",400,46.4)] {
            let center=map.triangles.iter().filter(|t|t.section==model).map(|t| {
                std::array::from_fn::<_,3,_>(|axis|t.points.iter().map(|p|p[axis]).sum::<f32>()/3.0)
            }).min_by(|a,b|(a[1]-target_y).abs().total_cmp(&(b[1]-target_y).abs())).unwrap();
            poses.push((name,center));
        }
        let config = Config { trail: false, rotate: true, compass: true,
            hide_in_boss_fights: false, hide_in_caves: true, dungeon_mode: true, outdoor_layers:true, parchment_maps:true,
            dungeon_height_band: 2.0, dungeon_floor_key: None, size: 1.0,
            margin: 24.0, toggle_key: None, zoom_key: None };
        let mut minimap = Minimap::new(Path::new("missing-test-game-directory"), config, Some(map));
        minimap.alpha = 1.0;
        minimap.paper=Some(TextureId::new(1));
        let mut context = Context::create();
        context.set_ini_filename(None);
        context.io_mut().display_size = [800.,600.];
        context.io_mut().delta_time = 1./60.;
        let atlas=context.fonts().build_rgba32_texture();
        if let Some(dir)=std::env::var_os("MINIMAP_PREVIEW_DIR") {
            let dir=Path::new(&dir);std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("atlas.rgba"),atlas.data).unwrap();
            std::fs::write(dir.join("atlas.json"),format!("{{\"width\":{},\"height\":{}}}",atlas.width,atlas.height)).unwrap();
        }
        for (name,position) in poses {
        let player=player_at(position);
        assert!(minimap.dungeon_active(&player));
        for view in [View::Layered,View::Current,View::All] {
            minimap.dungeon_view = view;
            {
                let ui = context.frame();
                let draw = ui.get_foreground_draw_list();
                minimap.draw_dungeon(ui,&draw,&player,[400.,300.],96.,1.);
            }
            let data = context.render();
            if let Some(dir)=std::env::var_os("MINIMAP_PREVIEW_DIR") {
                dump_scene(&Path::new(&dir).join(format!("{name}-{view:?}.json")),data);
            }
            assert!(data.total_vtx_count > 0 && data.total_idx_count > 0);
            for list in data.draw_lists() {
                for v in list.vtx_buffer() {
                    assert!(v.pos.iter().all(|x| x.is_finite()));
                    let d = [v.pos[0]-400.,v.pos[1]-300.];
                    assert!(d[0]*d[0]+d[1]*d[1] <= 98.*98.);
                }
            }
        }
        }
    }

    #[test]
    #[ignore="Requires DUNGEON_NAV_TEST_DIR generated from the local game"]
    fn all_small_dungeons_render_at_multiple_heights_views_and_zooms() {
        let _lock=IMGUI_TEST_LOCK.lock().unwrap();
        let directory=std::env::var_os("DUNGEON_NAV_TEST_DIR").expect("Set local dungeon directory");
        let mut paths:Vec<_>=std::fs::read_dir(&directory).unwrap().map(|e|e.unwrap().path())
            .filter(|p|p.extension().is_some_and(|e|e=="nav")).collect();
        paths.sort();assert_eq!(paths.len(),60);
        let mut minimap=Minimap::new(Path::new("missing-test-game-directory"),test_config(),None);
        minimap.alpha=1.;
        minimap.paper=Some(TextureId::new(1));
        let mut context=Context::create();context.set_ini_filename(None);
        context.io_mut().display_size=[800.,600.];context.io_mut().delta_time=1./60.;
        context.io_mut().backend_flags.insert(hudhook::imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        context.fonts().build_rgba32_texture();
        for path in paths {
            let map=DungeonMap::load(&path).unwrap();
            let block=map.block;
            let mut positions:Vec<[f32;3]>=map.triangles.iter().map(|triangle|
                std::array::from_fn(|axis|triangle.points.iter().map(|p|p[axis]).sum::<f32>()/3.)).collect();
            positions.sort_by(|a,b|a[1].total_cmp(&b[1]));
            let samples=[positions[0],positions[positions.len()/2],*positions.last().unwrap()];
            minimap.dungeon=Some(map);minimap.dungeon_floor.reset();
            for (pose,position) in samples.into_iter().enumerate() {
                let mut player=player_at(position);player.block_id=block;
                assert!(minimap.dungeon_active(&player));
                for view in [View::Layered,View::Current,View::All] {
                    minimap.dungeon_view=view;
                    for zoom in [0,2] {
                        minimap.zoom=zoom;
                        {
                            let ui=context.frame();let draw=ui.get_foreground_draw_list();
                            minimap.draw_dungeon(ui,&draw,&player,[400.,300.],96.,1.);
                        }
                        let data=context.render();
                        assert!(data.total_vtx_count>0,"no geometry for {path:?} {position:?}");
                        assert!(data.draw_lists().map(|l|l.commands().count()).sum::<usize>()<64,"unbatched paper draws: {path:?}");
                        for list in data.draw_lists() {for v in list.vtx_buffer() {
                            assert!(v.pos.iter().all(|x|x.is_finite()),"nonfinite {path:?}");
                            assert!((v.pos[0]-400.).hypot(v.pos[1]-300.)<=98.,"outside frame {path:?}");
                        }}
                        if pose==1 && view==View::Layered && zoom==0 {
                            if let Some(dir)=std::env::var_os("MINIMAP_PREVIEW_DIR") {
                                let name=path.file_stem().unwrap().to_string_lossy();
                                dump_scene(&Path::new(&dir).join(format!("{name}.json")),data);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    #[ignore="Requires DUNGEON_NAV_TEST_DIR generated from the local game"]
    fn dungeon_selection_clears_old_geometry_during_rapid_warps() {
        use std::time::{Duration,Instant};
        let directory=std::env::var_os("DUNGEON_NAV_TEST_DIR").expect("Set local dungeon directory");
        let directory=Path::new(&directory);
        let loader=DungeonLoader::spawn(directory,&directory.join("selection-test.log")).unwrap();
        let mut minimap=Minimap::new(Path::new("missing-test-game-directory"),test_config(),None)
            .with_dungeon_loader(Some(loader));
        let wait=|minimap:&mut Minimap,block:[u8;4]| {
            let deadline=Instant::now()+Duration::from_secs(10);
            loop {
                minimap.select_dungeon(block);
                if minimap.dungeon.as_ref().is_some_and(|map|map.block==block) {break;}
                assert!(Instant::now()<deadline,"map load timed out: {block:?}");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        wait(&mut minimap,[30,0,0,0]);
        minimap.select_dungeon([30,1,0,0]);
        assert!(minimap.dungeon.as_ref().is_none_or(|map|map.block==[30,1,0,0]));
        minimap.select_dungeon([31,3,0,0]);
        assert!(minimap.dungeon.as_ref().is_none_or(|map|map.block==[31,3,0,0]));
        wait(&mut minimap,[31,3,0,0]);
        assert_eq!(minimap.dungeon_name,"Groveside Cave");
        minimap.select_dungeon([60,42,36,0]);
        assert!(minimap.dungeon.is_none());
        assert_eq!(minimap.dungeon_floor.height(),None);
        wait(&mut minimap,[43,1,0,0]);
        assert_eq!(minimap.dungeon_name,"Dragon's Pit");
    }

    #[test]
    #[ignore="Requires OUTDOOR_NAV_TEST_DIR from the installed game"]
    fn real_outdoor_lower_and_upper_layers_render_separately() {
        use std::time::{Instant,Duration};
        let _lock=IMGUI_TEST_LOCK.lock().unwrap();
        let dir=std::env::var_os("OUTDOOR_NAV_TEST_DIR").expect("Set outdoor fixtures");let dir=Path::new(&dir);
        let loader=outdoor::Loader::spawn(dir,&dir.join("render-test.log")).unwrap();
        let mut minimap=Minimap::new(Path::new("missing-test-game"),test_config(),None).with_outdoor_loader(Some(loader));
        minimap.alpha=1.0;
        minimap.paper=Some(TextureId::new(1));
        let mut context=Context::create();context.set_ini_filename(None);
        context.io_mut().display_size=[800.,600.];context.io_mut().delta_time=1./60.;
        // Match hudhook's DX12 backend, including >64k-vertex draw-list support.
        context.io_mut().backend_flags.insert(hudhook::imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        let atlas=context.fonts().build_rgba32_texture();
        if let Some(path)=std::env::var_os("OUTDOOR_PREVIEW_DIR") {
            let path=Path::new(&path);std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join("atlas.rgba"),atlas.data).unwrap();
            std::fs::write(path.join("atlas.json"),format!("{{\"width\":{},\"height\":{}}}",atlas.width,atlas.height)).unwrap();
        }
        let mut frames=0;let mut elapsed=Duration::ZERO;
        for (id,name) in [("m60_34_42_00","Village"),("m61_47_43_00","Ellac"),("m61_45_46_00","Rauh")] {
            let map=DungeonMap::load(&dir.join(format!("{id}.nav"))).unwrap();
            let (lower,gap)=outdoor::lower_sample(&map).expect("lower outdoor path");
            let mut player=player_at(lower);player.block_id=map.block;player.position=[lower[0],lower[2]];
            player.space=map.block[0] as i32;player.in_cave=false;
            player.local_position=[lower[0]-map.block[1] as f32*256.,lower[1],lower[2]-map.block[2] as f32*256.];
            let deadline=Instant::now()+Duration::from_secs(15);
            while minimap.outdoor.as_ref().is_none_or(|w|w.key!=outdoor::key(&player).unwrap()) {
                minimap.select_outdoor(&player,0.1);assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));
            }
            for _ in 0..5 {minimap.select_outdoor(&player,0.1);}
            minimap.outdoor_mode=outdoor::Mode::Auto;assert!(minimap.outdoor_active(&player));
            minimap.outdoor_mode=outdoor::Mode::Surface;assert!(!minimap.outdoor_active(&player));
            minimap.outdoor_mode=outdoor::Mode::Local;assert!(minimap.outdoor_active(&player));
            for (level,point) in [("Lower",lower),("Upper",[lower[0],lower[1]+gap,lower[2]])] {
                for zoom in 0..3 {
                    let start=Instant::now();
                    let window=minimap.outdoor.as_ref().unwrap();
                    let floor=window.map.floor_height(point);
                    {
                        let ui=context.frame();let draw=ui.get_foreground_draw_list();
                        minimap.draw_geometry(ui,&draw,&window.map,point,floor,ZOOM_RANGES[zoom],outdoor::HEIGHT_BAND,
                            View::Layered,outdoor::CONTEXT_EXTENT,[400.,300.],96.,1.0);
                    }
                    let data=context.render();elapsed+=start.elapsed();frames+=1;
                    assert!(data.total_vtx_count>0);
                    assert!(data.draw_lists().map(|l|l.commands().count()).sum::<usize>()<64,"unbatched outdoor paper draws: {id}");
                    // Fine dark ink may cross the exact cursor pixel; the surrounding
                    // supporting path must still contain readable warm current-level paper.
                    let mut warmest=[0.;3];
                    for dx in [-2.,0.02,2.] {for dy in [-2.,0.03,2.] {
                        let pixel=sample(data,[400.+dx,300.+dy]);
                        if pixel[0]>warmest[0] {warmest=pixel;}
                    }}
                    let background=paper_sample(minimap.paper_uv([0.,0.],point,96./ZOOM_RANGES[zoom]))[0]*99.;
                    assert!(warmest[0]>background+25. && warmest[0]>warmest[2],"{id} {level}: {warmest:?}, background={background}");
                    for list in data.draw_lists(){for v in list.vtx_buffer(){
                        assert!(v.pos.iter().all(|v|v.is_finite()));assert!((v.pos[0]-400.).hypot(v.pos[1]-300.)<=98.0);
                    }}
                    if zoom==1 {if let Some(path)=std::env::var_os("OUTDOOR_PREVIEW_DIR") {
                        dump_scene(&Path::new(&path).join(format!("{name}-{level}.json")),data);
                    }}
                }
            }
            println!("Outdoor {id}: lower={lower:?}, upper gap={gap:.1} m");
        }
        println!("Outdoor {frames} frames: {:.2} ms per frame (CPU geometry only)",elapsed.as_secs_f64()*1000./frames as f64);
        let mut other=player_at([0.,0.,0.]);other.block_id=[30,0,0,0];minimap.select_outdoor(&other,0.1);
        assert!(minimap.outdoor.is_none());assert!(!minimap.outdoor_active(&other));
    }
}

pub struct Minimap {
    dungeon: Option<DungeonMap>,
    dungeon_loader: Option<DungeonLoader>,
    dungeon_block: Option<[u8;4]>,
    dungeon_name: String,
    dungeon_view: View,
    dungeon_floor: FloorTracker,
    outdoor_loader:Option<outdoor::Loader>,
    outdoor_key:Option<outdoor::Key>,
    outdoor:Option<outdoor::Window>,
    outdoor_mode:outdoor::Mode,
    outdoor_floor:FloorTracker,
    outdoor_coverage:outdoor::Coverage,
    config: Config,
    loader: Loader,
    frame: Option<TextureId>,
    arrow: Option<TextureId>,
    beacon: Option<TextureId>,
    paper: Option<TextureId>,
    map_texture: Option<TextureId>,
    /// What `map_texture` currently shows.
    map_window: Option<(MapId, [i32; 2])>,
    requested_window: Option<(MapId, [i32; 2])>,
    font_display_height: f32,
    last: Option<Snapshot>,
    camera_forward: [f32; 2],
    camera_right: [f32; 2],
    trail: VecDeque<[f32; 2]>,
    trail_space: i32,
    visible: bool,
    zoom: usize,
    alpha: f32,
    /// Displayed facing as a world angle, where 0 is north (+Z).
    facing_yaw: Option<f32>,
}

impl Minimap {
    pub fn new(game_dir: &Path, config: Config, dungeon: Option<DungeonMap>) -> Self {
        Self {
            dungeon,
            dungeon_loader:None,
            dungeon_block:None,
            dungeon_name:"Tombsward Catacombs".to_owned(),
            dungeon_view: View::Layered,
            dungeon_floor: FloorTracker::default(),
            outdoor_loader:None,
            outdoor_key:None,
            outdoor:None,
            outdoor_mode:outdoor::Mode::Auto,
            outdoor_floor:FloorTracker::default(),
            outdoor_coverage:outdoor::Coverage::default(),
            config,
            loader: Loader::spawn(game_dir),
            frame: None,
            arrow: None,
            beacon: None,
            paper: None,
            map_texture: None,
            map_window: None,
            requested_window: None,
            font_display_height: 1080.0,
            last: None,
            camera_forward: [0.0, 1.0],
            camera_right: [1.0, 0.0],
            trail: VecDeque::new(),
            trail_space: 0,
            visible: true,
            zoom: 1,
            alpha: 0.0,
            facing_yaw: None,
        }
    }

    pub fn with_dungeon_loader(mut self, loader:Option<DungeonLoader>) -> Self {
        self.dungeon_loader=loader;
        self
    }

    pub fn with_outdoor_loader(mut self,loader:Option<outdoor::Loader>)->Self {
        self.outdoor_loader=loader;self
    }

    fn select_outdoor(&mut self,player:&Snapshot,dt:f32) {
        let Some(loader)=&self.outdoor_loader else {return;};
        let wanted=outdoor::key(player);
        if wanted!=self.outdoor_key {
            self.outdoor_key=wanted;
            if !wanted.is_some_and(|key|self.outdoor.as_ref().is_some_and(|w|w.covers(key))) {
                self.outdoor=None;self.outdoor_floor.reset();self.outdoor_coverage=outdoor::Coverage::default();
            }
            if let Some(key)=wanted {loader.request(key);}
        }
        if let Some((key,result))=loader.poll() {
            if Some(key)==self.outdoor_key {self.outdoor=result.ok();}
        }
        if let Some(window)=&self.outdoor {
            let point=outdoor::position(player);
            self.outdoor_coverage.update_window(window,point,dt);
            self.outdoor_floor.update_height(self.outdoor_coverage.floor.unwrap_or(point[1]),point,dt);
        }
    }

    fn outdoor_active(&self,player:&Snapshot)->bool {
        outdoor::position(player).iter().all(|v|v.is_finite())
            && self.outdoor.as_ref().is_some_and(|w|outdoor::key(player).is_some_and(|key|w.covers(key)))
            && self.outdoor_coverage.active(self.outdoor_mode)
    }

    fn select_dungeon(&mut self, block:[u8;4]) {
        let Some(loader)=&self.dungeon_loader else {return;};
        if self.dungeon_block!=Some(block) {
            self.dungeon_block=Some(block);
            self.dungeon=None;
            self.dungeon_floor.reset();
            loader.request(block);
        }
        if let Some(loaded)=loader.poll() {
            // A warp can finish while another map is still being decoded.
            if Some(loaded.block)==self.dungeon_block {
                self.dungeon=loaded.map.ok();
                self.dungeon_name=loaded.name;
            }
        }
    }
}

impl ImguiRenderLoop for Minimap {
    fn initialize<'a>(&'a mut self, ctx: &mut Context, render_context: &'a mut dyn RenderContext) {
        ctx.set_ini_filename(None);
        self.font_display_height = ctx.io().display_size[1];
        // The first font added becomes the default one.
        let path = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default())
            .join(r"Fonts\georgiab.ttf");
        if let Ok(data) = std::fs::read(path) {
            ctx.fonts().add_font(&[FontSource::TtfData {
                data: &data,
                size_pixels: FONT_SIZE * self.font_display_height / 1080.0,
                config: None,
            }]);
        }
        self.frame = load_png(render_context, FRAME_PNG);
        self.arrow = load_png(render_context, ARROW_PNG);
        self.beacon = load_png(render_context, BEACON_PNG);
        if self.config.parchment_maps { self.paper = load_png(render_context, PAPER_PNG); }
    }

    fn before_render<'a>(
        &'a mut self,
        ctx: &mut Context,
        render_context: &'a mut dyn RenderContext,
    ) {
        // The font is baked once, so follow later resolution changes by scaling it.
        let height = ctx.io().display_size[1];
        ctx.io_mut().font_global_scale = height / self.font_display_height * self.config.size;

        // hudhook uploads on the game's queue and waits for the GPU. On loading screens that
        // queue can be waiting on the game, which deadlocks it.
        if !self.last.as_ref().is_some_and(in_control) {
            return;
        }
        let Some(window) = self.loader.poll() else {
            return;
        };
        let size = WINDOW as u32;
        let uploaded = match self.map_texture {
            Some(texture) => render_context
                .replace_texture(texture, &window.rgba, size, size)
                .is_ok(),
            None => match render_context.load_texture(&window.rgba, size, size) {
                Ok(texture) => {
                    self.map_texture = Some(texture);
                    true
                }
                Err(_) => false,
            },
        };
        if uploaded {
            self.map_window = Some((window.map, window.origin));
        } else {
            // Ask again next frame instead of waiting until the player nears the window's edge.
            self.requested_window = None;
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        let pressed = |key: Option<_>| key.is_some_and(|key| ui.is_key_pressed_no_repeat(key));
        if pressed(self.config.toggle_key) {
            self.visible = !self.visible;
        }
        if pressed(self.config.zoom_key) {
            self.zoom = (self.zoom + 1) % ZOOM_RANGES.len();
        }
        if pressed(self.config.dungeon_floor_key) {
            if game::LATEST.lock().unwrap().as_ref().is_some_and(|p|outdoor::key(p).is_some()) {
                self.outdoor_mode=self.outdoor_mode.next();
            } else {self.dungeon_view = self.dungeon_view.next();}
        }

        let snapshot = game::LATEST.lock().unwrap().clone();
        if let Some(snapshot) = &snapshot {
            self.select_dungeon(snapshot.block_id);
            self.select_outdoor(snapshot,ui.io().delta_time);
            self.update(snapshot);
            if self.dungeon_active(snapshot) {
                self.dungeon_floor.update(self.dungeon.as_ref().unwrap(),snapshot.local_position,ui.io().delta_time);
            } else {self.dungeon_floor.reset();}
        }
        // Characters turn instantly, so the arrow eases toward the facing in world space,
        // which keeps it from lagging behind camera turns.
        if let Some([x, z]) = snapshot.as_ref().and_then(|s| normalize(s.facing)) {
            let target = x.atan2(z);
            let current = self.facing_yaw.get_or_insert(target);
            *current += wrap_angle(target - *current) * (ui.io().delta_time * 14.0).min(1.0);
        }
        // Menus like the pause menu and world map draw where the minimap sits.
        let target = match &snapshot {
            Some(snapshot)
                if self.visible
                    && in_control(snapshot)
                    && !(self.config.hide_in_boss_fights && snapshot.boss_fight)
                    && !(self.config.hide_in_caves && snapshot.in_cave && !self.dungeon_active(snapshot)) =>
            {
                1.0
            }
            _ => 0.0,
        };
        self.alpha += (target - self.alpha) * (ui.io().delta_time * 12.0).min(1.0);
        // The last known state keeps drawing while the map fades out.
        if self.alpha < 0.01 {
            return;
        }
        if let Some(last) = &self.last {
            self.draw(ui, last);
        }
    }
}

impl Minimap {
    fn dungeon_active(&self, player: &Snapshot) -> bool {
        player.local_position.iter().all(|v| v.is_finite())
            && self.dungeon.as_ref().is_some_and(|map| map.block == player.block_id)
    }

    fn update(&mut self, snapshot: &Snapshot) {
        self.last = Some(snapshot.clone());
        // Without rotation the basis stays at its north-up default.
        if self.config.rotate {
            if let Some(forward) = normalize(snapshot.camera_forward) {
                self.camera_forward = forward;
            }
            if let Some(right) = normalize(snapshot.camera_right) {
                self.camera_right = right;
            }
        }

        // hudhook leaks the staging buffer of every upload, so only re-center the window near
        // its edge instead of on every cell crossing.
        if let Some(map) = snapshot.map.filter(|_| !self.dungeon_active(snapshot)) {
            let pixel = maptiles::map_pixel(snapshot.position);
            let near_edge = match self.requested_window {
                Some((requested_map, origin)) if requested_map == map => (0..2).any(|axis| {
                    let offset = pixel[axis] - (origin[axis] * TILE as i32) as f32;
                    !(WINDOW_MARGIN..WINDOW as f32 - WINDOW_MARGIN).contains(&offset)
                }),
                _ => true,
            };
            if near_edge {
                let cell = pixel.map(|p| (p / TILE as f32).floor() as i32);
                let wanted = (map, [cell[0] - 1, cell[1] - 1]);
                self.loader.request(wanted.0, wanted.1);
                self.requested_window = Some(wanted);
            }
        }

        if snapshot.space != self.trail_space {
            self.trail.clear();
            self.trail_space = snapshot.space;
        }
        if let Some(&last) = self.trail.back() {
            let distance = length([
                snapshot.position[0] - last[0],
                snapshot.position[1] - last[1],
            ]);
            if distance > TELEPORT_DISTANCE {
                self.trail.clear();
            } else if distance < TRAIL_SPACING {
                return;
            }
        }
        self.trail.push_back(snapshot.position);
        if self.trail.len() > TRAIL_LENGTH {
            self.trail.pop_front();
        }
    }

    /// Turns a horizontal world direction into a screen direction. The map turns with
    /// the camera: whatever the camera faces is up.
    fn to_screen(&self, [dx, dz]: [f32; 2]) -> [f32; 2] {
        let [rx, rz] = self.camera_right;
        let [fx, fz] = self.camera_forward;
        [dx * rx + dz * rz, -(dx * fx + dz * fz)]
    }

    /// The inverse of `to_screen`, since camera right and forward are orthonormal on the ground.
    fn to_world(&self, [sx, sy]: [f32; 2]) -> [f32; 2] {
        let [rx, rz] = self.camera_right;
        let [fx, fz] = self.camera_forward;
        [sx * rx - sy * fx, sx * rz - sy * fz]
    }

    /// World-anchored material coordinates keep the paper fixed when walking or turning.
    fn paper_uv(&self,screen:[f32;2],position:[f32;3],scale:f32)->[f32;2] {
        let world=self.to_world(screen.map(|v|v/scale));
        [(world[0]+position[0])/PAPER_PERIOD,(world[1]+position[2])/PAPER_PERIOD]
    }

    fn paint_polygon(&self,draw:&DrawListMut,points:&[[f32;2]],position:[f32;3],scale:f32,center:[f32;2],tint:[u8;3]) {
        let Some(texture)=self.paper else {return;};
        if points.len()<3 {return;}
        let screen=|p:[f32;2]|[p[0]+center[0],p[1]+center[1]];
        let uv=|p|self.paper_uv(p,position,scale);
        // The fourth vertex repeats the third; only the first triangle has area.
        // hudhook's wrap sampler lets adjacent clipped polygons share one material.
        for i in 1..points.len()-1 {
            let [a,b,c]=[points[0],points[i],points[i+1]];
            draw.add_image_quad(texture,screen(a),screen(b),screen(c),screen(c))
                .uv(uv(a),uv(b),uv(c),uv(c)).col(self.color(tint,1.0)).build();
        }
    }

    /// Screen direction of a compass bearing, where 0 is north (+Z) and 90 is east (+X).
    fn bearing(&self, degrees: f32) -> [f32; 2] {
        let (sin, cos) = degrees.to_radians().sin_cos();
        self.to_screen([sin, cos])
    }

    fn color(&self, [r, g, b]: [u8; 3], opacity: f32) -> ImColor32 {
        ImColor32::from_rgba(
            r,
            g,
            b,
            ((opacity * self.alpha).clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    fn draw(&self, ui: &Ui, player: &Snapshot) {
        let [width, height] = ui.io().display_size;
        let screen_scale = height / 1080.0;
        let s = screen_scale * self.config.size;
        let radius = RADIUS * s;
        let frame_size = radius / FRAME_HOLE;
        let margin = self.config.margin * screen_scale;
        let center = [width - margin - frame_size / 2.0, margin + frame_size / 2.0];
        let draw_list = ui.get_foreground_draw_list();

        draw_list
            .add_circle(center, frame_size * 0.47, self.color(BLACK, 0.35))
            .filled(true)
            .num_segments(96)
            .build();
        let dungeon_active = self.dungeon_active(player);
        let outdoor_active = self.outdoor_active(player);
        let layout_active=dungeon_active || outdoor_active;
        if layout_active || !self.draw_map(&draw_list, player, center, radius) {
            draw_list
                .add_circle(center, radius, self.color(BACKGROUND, 0.9))
                .filled(true)
                .num_segments(96)
                .build();
        }
        if dungeon_active {
            self.draw_dungeon(ui,&draw_list, player, center, radius, s);
        } else if outdoor_active {
            let window=self.outdoor.as_ref().unwrap();
            let point=outdoor::position(player);
            let floor=self.outdoor_floor.height().unwrap_or(point[1]);
            self.draw_geometry(ui,&draw_list,&window.map,point,floor,ZOOM_RANGES[self.zoom],
                outdoor::HEIGHT_BAND,View::Layered,outdoor::CONTEXT_EXTENT,center,radius,s);
        }
        // Darken toward the rim so the frame sits on the map instead of cutting it.
        for i in 0..4 {
            draw_list
                .add_circle(
                    center,
                    radius - (6.0 + i as f32 * 3.0) * s,
                    self.color(BLACK, 0.22 - i as f32 * 0.05),
                )
                .num_segments(96)
                .thickness(4.0 * s)
                .build();
        }

        if self.config.trail && !layout_active {
            self.draw_trail(&draw_list, player, center, radius, s);
        }
        if !layout_active {
            self.draw_points(&draw_list, player, center, radius, s);
            self.draw_graces(&draw_list, player, center, radius, s);
            self.draw_beacons(ui, &draw_list, player, center, radius, s);
        }

        let letters = if self.config.compass {
            ["N", "E", "S", "W"].as_slice()
        } else {
            &[]
        };
        for (i, letter) in letters.iter().enumerate() {
            let direction = self.bearing(i as f32 * 90.0);
            let position = [
                center[0] + direction[0] * (radius - 14.0 * s),
                center[1] + direction[1] * (radius - 14.0 * s),
            ];
            draw_list
                .add_circle(position, 9.0 * s, self.color(BACKGROUND, 0.75))
                .filled(true)
                .build();
            let color = if i == 0 {
                self.color(BRIGHT_GOLD, 1.0)
            } else {
                self.color(PALE_GOLD, 0.9)
            };
            self.draw_text_centered(ui, &draw_list, position, color, letter);
        }

        self.draw_arrow(&draw_list, center, s);
        if dungeon_active {
            let y=center[1]+frame_size*0.5+8.0*s;
            let mut title=if self.dungeon_view==View::All {format!("{} - all levels",self.dungeon_name)} else {self.dungeon_name.clone()};
            if ui.calc_text_size(&title)[0]>frame_size {
                while !title.is_empty() && ui.calc_text_size(format!("{title}..."))[0]>frame_size {title.pop();}
                title.push_str("...");
            }
            self.draw_text_centered(ui,&draw_list,[center[0],y],self.color(PALE_GOLD,1.0),&title);
            if self.dungeon_view==View::Current {
                self.draw_text_centered(ui,&draw_list,[center[0],y+15.0*s],self.color(LEVEL_EDGE,0.9),"Current level");
            } else {
                for (x,text,color) in [(-45.0,"UP",UPPER_EDGE),(0.0,"HERE",LEVEL_EDGE),(45.0,"DOWN",LOWER_EDGE)] {
                    self.draw_text_centered(ui,&draw_list,[center[0]+x*s,y+15.0*s],self.color(color,1.0),text);
                }
            }
        }

        if outdoor::key(player).is_some() && self.outdoor_loader.is_some() {
            let y=center[1]+frame_size*0.5+8.0*s;
            let mode=match self.outdoor_mode {outdoor::Mode::Auto=>"Auto",outdoor::Mode::Local=>"Local",outdoor::Mode::Surface=>"Surface"};
            let title=if outdoor_active {
                if let Some(gap)=self.outdoor_coverage.above {format!("Lower path | upper +{gap:.0}m")} else {"Local ground layout".to_owned()}
            } else if self.outdoor_mode==outdoor::Mode::Local {"Local layout unavailable".to_owned()} else {"Surface map".to_owned()};
            self.draw_text_centered(ui,&draw_list,[center[0],y],self.color(PALE_GOLD,1.0),&title);
            let hint=self.config.dungeon_floor_key.map_or_else(||mode.to_owned(),|key|format!("{mode} | {key:?}: change view"));
            self.draw_text_centered(ui,&draw_list,[center[0],y+15.0*s],self.color(LEVEL_EDGE,0.9),&hint);
            if outdoor_active {
                let name=self.outdoor.as_ref().unwrap().name.split(';').next().unwrap_or_default();
                let mut name=name.rsplit(" - ").next().unwrap_or(name).to_owned();
                while !name.is_empty() && ui.calc_text_size(&name)[0]>frame_size {name.pop();}
                self.draw_text_centered(ui,&draw_list,[center[0],y+30.0*s],self.color(PALE_GOLD,0.8),&name);
            }
        }

        if let Some(frame) = self.frame {
            let half = frame_size / 2.0;
            draw_list
                .add_image(
                    frame,
                    [center[0] - half, center[1] - half],
                    [center[0] + half, center[1] + half],
                )
                .col(self.color([255, 255, 255], 1.0))
                .build();
        }
    }

    fn draw_dungeon(&self, ui:&Ui, draw: &DrawListMut, player: &Snapshot, center: [f32; 2], radius: f32, s: f32) {
        let Some(map) = &self.dungeon else { return; };
        let range = dungeon::ZOOM[self.zoom];
        let floor = self.dungeon_floor.height().unwrap_or_else(||map.floor_height(player.local_position));
        self.draw_geometry(ui,draw,map,player.local_position,floor,range,self.config.dungeon_height_band,
            self.dungeon_view,8.0,center,radius,s);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_geometry(&self,ui:&Ui,draw:&DrawListMut,map:&DungeonMap,position:[f32;3],floor:f32,
        range:f32,band:f32,view:View,context_extent:f32,center:[f32;2],radius:f32,s:f32) {
        let scale=radius/range;
        let painted=self.config.parchment_maps && self.paper.is_some();
        if painted {
            let disc:Vec<_>=(0..DISC_SEGMENTS).map(|i| {
                let angle=TAU*i as f32/DISC_SEGMENTS as f32;
                [angle.cos()*(radius-s),angle.sin()*(radius-s)]
            }).collect();
            self.paint_polygon(draw,&disc,position,scale,center,[99,91,74]);
        }
        let low = floor - band;
        let high = floor + band;
        let project = |p: [f32; 3]| {
            self.to_screen([p[0] - position[0], p[2] - position[2]]).map(|v| v * scale)
        };
        let extent=if view==View::All {1_000_000.0} else {context_extent};
        let mut layers=Vec::new();
        if view!=View::Current {
            layers.push((floor-extent,low,LOWER_FILL,LOWER_EDGE,0.75));
            layers.push((high,floor+extent,UPPER_FILL,UPPER_EDGE,0.85));
        }
        layers.push((low,high,LEVEL_FILL,LEVEL_EDGE,1.45));
        // Complete each background level before drawing the bright current level on top.
        for (bottom,top,fill,edge,width) in layers {
        let current=bottom==low && top==high;
        let tint=if current {[246,236,208]} else if top==low {[113,101,83]} else {[104,140,182]};
        let mut contours=Vec::new();
        {
        let _opaque_mesh_edges=SolidMeshFill::foreground();
        for triangle in &map.triangles {
            if triangle.points.iter().all(|p|p[1]<bottom) || triangle.points.iter().all(|p|p[1]>top) {continue;}
            if (0..2).any(|axis| {
                let a = if axis == 0 { 0 } else { 2 };
                triangle.points.iter().all(|p| p[a] < position[a]-range) ||
                triangle.points.iter().all(|p| p[a] > position[a]+range)
            }) { continue; }
            let polygon=dungeon::clip_height(&triangle.points,bottom,top);
            if polygon.len() < 3 { continue; }
            let screen: Vec<_> = polygon.into_iter().map(project).collect();
            let mut clipped = dungeon::clip_disc(&screen, radius - s);
            if clipped.len() < 3 { continue; }
            // ImGui filled paths expect clockwise winding in screen coordinates.
            let area: f32 = (0..clipped.len()).map(|i| {
                let a=clipped[i]; let b=clipped[(i+1)%clipped.len()]; a[0]*b[1]-b[0]*a[1]
            }).sum();
            if area < 0.0 { clipped.reverse(); }
            if painted {
                self.paint_polygon(draw,&clipped,position,scale,center,tint);
                if current {
                    // Fixed world-height contours: no per-triangle shading or moving bands.
                    // Restrict to this slab; contours come from actual sloping geometry.
                    let mut height=(bottom/4.0).ceil()*4.0;
                    while height<top {
                        if let Some(line)=height_contour(triangle.points,height) {
                            if let Some(line)=dungeon::clip_segment_disc(line.map(project),radius-s) {
                                contours.push(line);
                            }
                        }
                        height+=4.0;
                    }
                }
            } else {
                for point in &mut clipped { point[0] += center[0]; point[1] += center[1]; }
                draw.add_polyline(clipped, self.color(fill, 1.0)).filled(true).build();
            }
        }
        }
        for mut line in contours {
            for p in &mut line {p[0]+=center[0];p[1]+=center[1];}
            draw.add_line(line[0],line[1],self.color([104,80,47],0.28)).thickness(0.6*s).build();
        }
        for &boundary in &map.boundaries {
            if [0,2].iter().any(|&axis|boundary.iter().all(|p|p[axis]<position[axis]-range)
                || boundary.iter().all(|p|p[axis]>position[axis]+range)) {continue;}
            let line=dungeon::clip_segment_height(boundary,bottom,top);
            let Some(line) = line else { continue; };
            let Some(mut line) = dungeon::clip_segment_disc(line.map(project), radius-s) else { continue; };
            for p in &mut line { p[0]+=center[0]; p[1]+=center[1]; }
            if painted && current {
                // Keep distant narrow paths open: screen-space strokes must not grow
                // wider than the navigation they describe when zooming out.
                let ink_width=(scale*0.4).clamp(0.35*s,0.9*s);
                let outdoor_map=map.block[0]>=60;
                if !outdoor_map && scale>=2.*s {
                    draw.add_line(line[0],line[1],self.color([48,36,21],0.22)).thickness(2.5*ink_width).build();
                }
                // Dense outdoor navigation has overlapping partition edges; softer
                // ochre ink keeps that detail from blacking out the ground below it.
                let ink=if outdoor_map {[144,111,69]} else {[74,56,33]};
                draw.add_line(line[0],line[1],self.color(ink,0.95)).thickness(ink_width).build();
            } else {
                draw.add_line(line[0], line[1], self.color(edge, 0.95)).thickness(width*s).build();
            }
        }
        }
        // Connected sloping patches give geometric up/down hints; they are not door markers.
        let mut hints:Vec<[f32;2]>=Vec::new();
        let mut label_boxes:Vec<([f32;2],[f32;2])>=Vec::new();
        for ramp in &map.ramps {
            // Only mark connections that reach the selected level, not unrelated remote stairs.
            if ramp.bottom[1]>high || ramp.top[1]<low {continue;}
            let rise=ramp.top[1]-ramp.bottom[1];
            let t=((floor-ramp.bottom[1])/rise).clamp(0.05,0.95);
            let point: [f32;3]=std::array::from_fn(|i|ramp.bottom[i]+(ramp.top[i]-ramp.bottom[i])*t);
            let origin=project(point);
            if origin[0].hypot(origin[1])>radius-28.0*s {continue;}
            if hints.iter().any(|p|(p[0]-origin[0]).hypot(p[1]-origin[1])<22.0*s) {continue;}
            let up=project(ramp.top);let down=project(ramp.bottom);
            let Some(direction)=normalize([up[0]-down[0],up[1]-down[1]]) else {continue;};
            let targets=[(ramp.top[1]-floor,1.0,UPPER_EDGE),(ramp.bottom[1]-floor,-1.0,LOWER_EDGE)];
            let mut drawn=false;
            for (delta,sign,color) in targets {
                if delta*sign<band+0.25 {continue;}
                let d=direction.map(|x|x*sign);
                let side=[-d[1],d[0]];
                let mut anchor=[center[0]+origin[0]+d[0]*5.0*s,center[1]+origin[1]+d[1]*5.0*s];
                if origin[0].hypot(origin[1])<12.0*s {for i in 0..2 {anchor[i]+=d[i]*10.0*s;}}
                let label=format!("{delta:+.0}m");
                let position=[anchor[0]+side[0]*8.0*s+d[0]*14.0*s,anchor[1]+side[1]*8.0*s+d[1]*14.0*s];
                let size=ui.calc_text_size(&label);
                let min=[position[0]-size[0]*0.5-2.0*s,position[1]-size[1]*0.5-2.0*s];
                let max=[position[0]+size[0]*0.5+2.0*s,position[1]+size[1]*0.5+2.0*s];
                if label_boxes.iter().any(|(a,b)|min[0]<b[0] && max[0]>a[0] && min[1]<b[1] && max[1]>a[1]) {continue;}
                if [min,[min[0],max[1]],max,[max[0],min[1]]].iter()
                    .any(|p|(p[0]-center[0]).hypot(p[1]-center[1])>radius-2.0*s) {continue;}
                for step in [0.0,4.0] {
                    let tip=[anchor[0]+d[0]*step*s,anchor[1]+d[1]*step*s];
                    let left=[tip[0]-d[0]*3.5*s+side[0]*3.0*s,tip[1]-d[1]*3.5*s+side[1]*3.0*s];
                    let right=[tip[0]-d[0]*3.5*s-side[0]*3.0*s,tip[1]-d[1]*3.5*s-side[1]*3.0*s];
                    draw.add_line(left,tip,self.color(BLACK,0.9)).thickness(4.0*s).build();
                    draw.add_line(tip,right,self.color(BLACK,0.9)).thickness(4.0*s).build();
                    draw.add_line(left,tip,self.color(color,1.0)).thickness(2.0*s).build();
                    draw.add_line(tip,right,self.color(color,1.0)).thickness(2.0*s).build();
                }
                self.draw_text_centered(ui,draw,[position[0]+s,position[1]+s],self.color(BLACK,0.9),&label);
                self.draw_text_centered(ui,draw,position,self.color(color,1.0),&label);
                label_boxes.push((min,max));
                drawn=true;
            }
            if drawn {hints.push(origin);}
        }
    }

    /// A fan of wedges clips the texture to a circle and gives each rim vertex its own
    /// rotated UV. Returns false when there's no map for this spot.
    fn draw_map(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
    ) -> bool {
        let (Some(texture), Some((map, origin)), Some(player_map)) =
            (self.map_texture, self.map_window, player.map)
        else {
            return false;
        };
        if map != player_map {
            return false;
        }
        let meters_per_pixel = ZOOM_RANGES[self.zoom] / radius;
        let player_pixel = maptiles::map_pixel(player.position);
        let uv = |screen_offset: [f32; 2]| {
            let world = self.to_world(screen_offset.map(|d| d * meters_per_pixel));
            let x = player_pixel[0] + world[0] - (origin[0] * TILE as i32) as f32;
            let y = player_pixel[1] + world[1] - (origin[1] * TILE as i32) as f32;
            [x / WINDOW as f32, 1.0 - y / WINDOW as f32]
        };
        let center_uv = uv([0.0, 0.0]);
        // A window from somewhere else would draw the wrong place until the new one arrives.
        if !center_uv.iter().all(|c| (0.0..=1.0).contains(c)) {
            return false;
        }
        let tint = self.color([236, 226, 210], 1.0);
        for i in 0..DISC_SEGMENTS {
            let rim = |k: usize| {
                let (sin, cos) = (k as f32 / DISC_SEGMENTS as f32 * TAU).sin_cos();
                [cos * radius, sin * radius]
            };
            let (a, b) = (rim(i), rim(i + 1));
            let (pa, pb) = (
                [center[0] + a[0], center[1] + a[1]],
                [center[0] + b[0], center[1] + b[1]],
            );
            draw_list
                .add_image_quad(texture, center, pa, pb, pb)
                .uv(center_uv, uv(a), uv(b), uv(b))
                .col(tint)
                .build();
        }
        true
    }

    fn draw_trail(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        let limit = radius - 6.0 * s;
        let project = |point: [f32; 2]| {
            let d = self.to_screen([point[0] - player.position[0], point[1] - player.position[1]]);
            [
                center[0] + d[0] * pixels_per_meter,
                center[1] + d[1] * pixels_per_meter,
            ]
        };
        let inside = |p: [f32; 2]| length([p[0] - center[0], p[1] - center[1]]) < limit;

        let points: Vec<[f32; 2]> = self
            .trail
            .iter()
            .map(|&p| project(p))
            .chain([center])
            .collect();
        for (i, segment) in points.windows(2).enumerate() {
            if !inside(segment[0]) || !inside(segment[1]) {
                continue;
            }
            let age = (i + 1) as f32 / (points.len() - 1) as f32;
            draw_list
                .add_line(
                    segment[0],
                    segment[1],
                    self.color([255, 236, 190], 0.10 + 0.55 * age),
                )
                .thickness(2.0 * s)
                .build();
        }
    }

    /// Dimmed once the game marks them done.
    fn draw_points(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        for point in &player.points {
            let d = self.to_screen([
                point.position[0] - player.position[0],
                point.position[1] - player.position[1],
            ]);
            if length(d) * pixels_per_meter > radius - 8.0 * s {
                continue;
            }
            let at = [
                center[0] + d[0] * pixels_per_meter,
                center[1] + d[1] * pixels_per_meter,
            ];
            let opacity = if point.cleared { 0.45 } else { 1.0 };
            let outline = self.color(BLACK, 0.8 * opacity);
            let dot = |r: f32, rgb: [u8; 3]| {
                draw_list
                    .add_circle(at, (r + 1.2) * s, outline)
                    .filled(true)
                    .build();
                draw_list
                    .add_circle(at, r * s, self.color(rgb, opacity))
                    .filled(true)
                    .build();
            };
            match point.kind {
                PointKind::Item => dot(2.4, [236, 190, 96]),
                PointKind::Npc => dot(3.0, [150, 196, 240]),
                PointKind::Invasion => dot(3.0, [214, 64, 52]),
                PointKind::Landmark => {
                    let r = 4.5 * s;
                    let diamond = vec![
                        [at[0], at[1] - r],
                        [at[0] + r, at[1]],
                        [at[0], at[1] + r],
                        [at[0] - r, at[1]],
                    ];
                    draw_list
                        .add_polyline(diamond.clone(), self.color([214, 190, 140], 0.85 * opacity))
                        .filled(true)
                        .build();
                    draw_list
                        .add_polyline([diamond.as_slice(), &diamond[..1]].concat(), outline)
                        .thickness(1.2 * s)
                        .build();
                }
                PointKind::Boss => {
                    let rgb = if point.cleared {
                        [150, 150, 150]
                    } else {
                        [200, 40, 36]
                    };
                    dot(4.0, [30, 10, 8]);
                    draw_list
                        .add_circle(at, 4.0 * s, self.color(rgb, opacity))
                        .thickness(1.8 * s)
                        .build();
                    draw_list
                        .add_circle(at, 1.6 * s, self.color(rgb, opacity))
                        .filled(true)
                        .build();
                }
                PointKind::Grace => {
                    draw_list
                        .add_circle(at, 4.5 * s, self.color([255, 222, 150], 0.8 * opacity))
                        .thickness(1.4 * s)
                        .build();
                }
            }
        }
    }

    /// Beacons out of range sit on the rim, pointing the way.
    fn draw_beacons(
        &self,
        ui: &Ui,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let Some(texture) = self.beacon else {
            return;
        };
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        let rim = radius - 18.0 * s;
        for beacon in &player.beacons {
            let d = self.to_screen([
                beacon.position[0] - player.position[0],
                beacon.position[1] - player.position[1],
            ]);
            let mut offset = [d[0] * pixels_per_meter, d[1] * pixels_per_meter];
            let distance = length(offset);
            let inside = distance <= rim;
            if !inside {
                offset = [offset[0] / distance * rim, offset[1] / distance * rim];
            }
            let at = [center[0] + offset[0], center[1] + offset[1]];
            let half = if inside { 13.0 } else { 10.0 } * s;
            let opacity = if inside { 1.0 } else { 0.8 };
            draw_list
                .add_image(
                    texture,
                    [at[0] - half, at[1] - half],
                    [at[0] + half, at[1] + half],
                )
                .col(self.color([255, 255, 255], opacity))
                .build();
            // Too small for a digit inside, so it goes beside it like on the game's compass.
            let number = beacon.number.to_string();
            let label = [at[0] + half * 0.85, at[1] + half * 0.35];
            self.draw_text_centered(
                ui,
                draw_list,
                label,
                self.color([255, 244, 220], opacity),
                &number,
            );
        }
    }

    /// A glowing golden point, like the grace icons on the world map.
    fn draw_graces(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        for grace in &player.graces {
            let d = self.to_screen([grace[0] - player.position[0], grace[1] - player.position[1]]);
            if length(d) * pixels_per_meter > radius - 10.0 * s {
                continue;
            }
            let at = [
                center[0] + d[0] * pixels_per_meter,
                center[1] + d[1] * pixels_per_meter,
            ];
            let circle = |r: f32, color: ImColor32| {
                draw_list.add_circle(at, r * s, color).filled(true).build();
            };
            circle(10.0, self.color([255, 210, 110], 0.12));
            circle(6.5, self.color([255, 214, 120], 0.25));
            draw_list
                .add_circle(at, 4.5 * s, self.color([255, 232, 170], 0.95))
                .thickness(1.6 * s)
                .build();
            circle(2.2, self.color([255, 248, 222], 1.0));
        }
    }

    fn draw_arrow(&self, draw_list: &DrawListMut, center: [f32; 2], s: f32) {
        let Some(arrow) = self.arrow else {
            return;
        };
        let (sin, cos) = self.facing_yaw.unwrap_or(0.0).sin_cos();
        let forward = self.to_screen([sin, cos]);
        let right = [-forward[1], forward[0]];
        let size = 30.0 * s;
        // Image corners, placed around the pivot with the image's up along `forward`.
        let corner = |u: f32, v: f32| {
            let (x, y) = ((u - ARROW_PIVOT[0]) * size, (v - ARROW_PIVOT[1]) * size);
            [
                center[0] + right[0] * x - forward[0] * y,
                center[1] + right[1] * x - forward[1] * y,
            ]
        };
        draw_list
            .add_image_quad(
                arrow,
                corner(0.0, 0.0),
                corner(1.0, 0.0),
                corner(1.0, 1.0),
                corner(0.0, 1.0),
            )
            .col(self.color([255, 255, 255], 1.0))
            .build();
    }

    fn draw_text_centered(
        &self,
        ui: &Ui,
        draw_list: &DrawListMut,
        center: [f32; 2],
        color: ImColor32,
        text: &str,
    ) {
        let [w, h] = ui.calc_text_size(text);
        let x = (center[0] - w * 0.5).round();
        let y = (center[1] - h * 0.5).round();
        draw_list.add_text([x + 1.0, y + 1.0], self.color(BLACK, 0.8), text);
        draw_list.add_text([x, y], color, text);
    }
}

/// Normal gameplay: no menu changing the HUD and no fade covering the screen.
fn in_control(snapshot: &Snapshot) -> bool {
    snapshot.hud_visible && snapshot.fade < 0.05
}

/// Intersect a sloping walkable triangle with an absolute-height plane.
fn height_contour(points:[[f32;3];3],height:f32)->Option<[[f32;3];2]> {
    let min=points.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min);
    let max=points.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max);
    if max-min<0.6 {return None;}
    let mut intersections=Vec::with_capacity(2);
    for i in 0..3 {
        let a=points[i];let b=points[(i+1)%3];
        // Half-open crossing avoids duplicate hits at a vertex.
        if (a[1]<=height && b[1]>height) || (b[1]<=height && a[1]>height) {
            let t=(height-a[1])/(b[1]-a[1]);
            intersections.push(std::array::from_fn(|axis|a[axis]+(b[axis]-a[axis])*t));
        }
    }
    if intersections.len()!=2 {return None;}
    let [a,b]=[intersections[0],intersections[1]];
    if (a[0]-b[0]).hypot(a[2]-b[2])<0.05 {None} else {Some([a,b])}
}

fn load_png(render_context: &mut dyn RenderContext, bytes: &[u8]) -> Option<TextureId> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).ok()?;
    render_context
        .load_texture(&rgba[..info.buffer_size()], info.width, info.height)
        .ok()
}

fn length([x, y]: [f32; 2]) -> f32 {
    (x * x + y * y).sqrt()
}

/// Wraps an angle difference into [-PI, PI) so easing takes the short way around.
fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn normalize(v: [f32; 2]) -> Option<[f32; 2]> {
    let l = length(v);
    (l > 1e-3).then(|| [v[0] / l, v[1] / l])
}
