"""Rasterize the actual ImGui test draw data; this is an offline preview, not a game screenshot.
Needs Pillow and NumPy. Set MINIMAP_PREVIEW_DIR when running the real-data renderer test,
then run: python tools/render-preview.py CAPTURE_DIRECTORY OUTPUT.png
"""
import argparse
import json
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw, ImageFont


def render(scene, atlas):
    paper=np.asarray(Image.open(Path(__file__).resolve().parents[1]/'assets/cartography-paper.png').convert('RGBA'),dtype=float)
    textures={0:atlas,1:paper}
    image=np.zeros((224,224,3),dtype=np.float32)
    image[:]=[24,20,15]
    for draw_list in scene['lists']:
        vertices=np.asarray(draw_list['vertices'],dtype=np.float64)
        indices=np.asarray(draw_list['indices'],dtype=np.int32)
        vertices[:,:2]-=[288.,188.]
        commands=draw_list.get('commands',[{'first':0,'count':len(indices),'texture':0}])
        triangles=[]
        for command in commands:
            first=command['first']; count=command['count']; texture_id=command['texture']
            triangles.extend((t,texture_id) for t in indices[first:first+count].reshape(-1,3))
        for ids,texture_id in triangles:
            triangle=vertices[ids]; texture_image=textures[texture_id]
            positions=triangle[:,:2]
            x0,y0=np.maximum(np.floor(positions.min(axis=0)).astype(int),0)
            x1,y1=np.minimum(np.ceil(positions.max(axis=0)).astype(int),[223,223])
            if x1<x0 or y1<y0: continue
            a,b,c=positions
            denominator=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
            if abs(denominator)<1e-8: continue
            yy,xx=np.mgrid[y0:y1+1,x0:x1+1]; xx=xx+0.5; yy=yy+0.5
            u=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/denominator
            v=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/denominator
            w=1-u-v
            inside=(u>=-1e-8)&(v>=-1e-8)&(w>=-1e-8)
            if not inside.any(): continue
            values=u[...,None]*triangle[0,2:]+v[...,None]*triangle[1,2:]+w[...,None]*triangle[2,2:]
            height,width=texture_image.shape[:2]
            tx=values[...,0]*width-0.5; ty=values[...,1]*height-0.5
            if texture_id!=1:
                tx=np.clip(tx,0,width-1);ty=np.clip(ty,0,height-1)
            ix=np.floor(tx).astype(int);iy=np.floor(ty).astype(int)
            fx=(tx-ix)[...,None];fy=(ty-iy)[...,None]
            if texture_id==1:
                ix%=width;iy%=height;jx=(ix+1)%width;jy=(iy+1)%height
            else:
                jx=np.minimum(ix+1,width-1);jy=np.minimum(iy+1,height-1)
            texture=(texture_image[iy,ix]*(1-fx)+texture_image[iy,jx]*fx)*(1-fy)+(texture_image[jy,ix]*(1-fx)+texture_image[jy,jx]*fx)*fy
            alpha=values[...,5:6]/255.0*texture[...,3:4]/255.0*inside[...,None]
            source=values[...,2:5]*texture[...,:3]/255.0
            target=image[y0:y1+1,x0:x1+1]
            target[:]=source*alpha+target*(1-alpha)
    result=Image.fromarray(np.uint8(np.clip(image,0,255)))
    draw=ImageDraw.Draw(result)
    draw.ellipse((15,15,209,209),outline=(132,109,63),width=1)
    draw.polygon([(112,105),(108,118),(112,115),(116,118)],fill=(255,224,156))
    return result.resize((448,448),Image.Resampling.LANCZOS)


def main(root, output, dungeons=False, outdoor=False, painted=False):
    info=json.loads((root/'atlas.json').read_text())
    atlas=np.frombuffer((root/'atlas.rgba').read_bytes(),dtype=np.uint8).reshape(info['height'],info['width'],4).astype(float)
    canvas=Image.new('RGB',(1456,1160 if dungeons or outdoor or painted else 636),(13,18,23)); draw=ImageDraw.Draw(canvas)
    font_path='C:/Windows/Fonts/segoeui.ttf'
    title=ImageFont.truetype(font_path,29); label=ImageFont.truetype(font_path,22); small=ImageFont.truetype(font_path,17)
    draw.text((28,18),'Pilot 6  |  Parchment, ink and height contours' if painted else 'Below-surface outdoor paths' if outdoor else 'Dungeon map blocks' if dungeons else 'Clean floors and height context',fill='white',font=title)
    panels=[('m30_01_00_00',"Impaler's Catacombs"),('m31_03_00_00','Groveside Cave'),
            ('m32_01_00_00','Limgrave Tunnels'),('m40_00_00_00','Fog Rift Catacombs'),
            ('m41_00_00_00','Belurat Gaol'),('m43_01_00_00',"Dragon's Pit")] if dungeons else [
            ('Entrance-Layered','Entrance level'),('Stairs-Layered','Stairs between levels'),('Lower-Layered','Lower rooms')]
    if outdoor:
        panels=[('Village-Lower','Albinauric village area - lower'),('Ellac-Lower','Ellac River area - lower'),
                ('Rauh-Lower','Rauh Base area - lower'),('Village-Upper','Village area - upper ground'),
                ('Ellac-Upper','Ellac area - upper ground'),('Rauh-Upper','Rauh area - upper ground')]
    if painted:
        panels=[('Entrance-Layered','Tombsward Catacombs'),('m31_03_00_00','Groveside Cave'),
                ('m43_01_00_00',"Dragon's Pit"),('Village-Lower','Albinauric village area - lower'),
                ('Ellac-Lower','Ellac River area - lower'),('Rauh-Lower','Rauh Base area - lower')]
    for i,(name,caption) in enumerate(panels):
        x=24+(i%3)*480; y=(i//3)*524
        draw.text((x+12,72+y),caption,fill=(222,207,172),font=label)
        scene=json.loads((root/f'{name}.json').read_text())
        canvas.paste(render(scene,atlas),(x,110+y))
    legend_y=1092 if dungeons or outdoor or painted else 568
    for x,text,color in [(280,'UPPER',(108,156,173)),(650,'CURRENT',(233,209,158)),(1035,'LOWER',(131,108,76))]:
        draw.rectangle((x,legend_y,x+12,legend_y+12),fill=color)
        draw.text((x+22,legend_y-8),text,fill=color,font=label)
    note='Offline preview of actual draw data. Original-style material; paths follow extracted navigation. Gameplay check pending.' if painted else 'Actual draw data: same X/Z at two heights in each column. Navigation surfaces; live alignment still needs testing.' if outdoor else 'Offline previews from actual ImGui draw data at sampled positions. New dungeon alignment needs an in-game check.' if dungeons else 'Offline preview from actual ImGui draw data. Gameplay check pending.'
    draw.text((28,legend_y+40),note,fill=(154,166,174),font=small)
    output.parent.mkdir(parents=True,exist_ok=True);canvas.save(output)
    print(output)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--dungeons',action='store_true',help='Show six sample dungeons from the 60-map render test')
    parser.add_argument('--outdoor',action='store_true',help='Compare real lower and upper outdoor layers')
    parser.add_argument('--painted',action='store_true',help='Show three dungeons and three below-surface outdoor layouts')
    args=parser.parse_args();main(args.root,args.output,args.dungeons,args.outdoor,args.painted)
