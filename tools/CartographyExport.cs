using System;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Text;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using SoulsFormats;

public sealed class CartographyExport
{
    private readonly List<(Vector3 a,Vector3 b,Vector3 c,byte kind)> faces=new();
    private readonly HashSet<string> seen=new();
    public int Count => faces.Count;
    public void Add(byte[] binder,float[] position,float[] rotation,float[] scale,float offsetX,float offsetZ)
    {
        // Single-axis MSB rotations are supported. Do not guess the ordering of
        // the few compound rotations; callers report those skipped placements.
        if(rotation.Count(v=>Math.Abs(v)>0.00001f)>1) throw new InvalidDataException("compound MSB rotation");
        float radians=(float)Math.PI/180f;
        var spin=Quaternion.CreateFromYawPitchRoll(rotation[1]*radians,rotation[0]*radians,rotation[2]*radians);
        var matrix=Matrix4x4.CreateScale(scale[0],scale[1],scale[2])*Matrix4x4.CreateFromQuaternion(spin)*
            Matrix4x4.CreateTranslation(position[0]+offsetX,position[1],position[2]+offsetZ);
        foreach(var file in BND4.Read(binder).Files.Where(f=>f.Name.EndsWith(".flver",StringComparison.OrdinalIgnoreCase))) {
            var flver=FLVER2.Read(file.Bytes.ToArray());
            foreach(var mesh in flver.Meshes) {
                if(mesh.FaceSets.Count==0) continue;
                var material=flver.Materials[mesh.MaterialIndex];
                byte kind=Regex.IsMatch(material.Name+" "+material.MTD,@"water|水面|流水|海水",RegexOptions.IgnoreCase)?(byte)1:(byte)0;
                var fs=mesh.FaceSets.FirstOrDefault(f=>(uint)f.Flags==0)??mesh.FaceSets[0];
                var indices=fs.Triangulate(false,false);
                for(int i=0;i+2<indices.Count;i+=3) {
                    int ia=indices[i],ib=indices[i+1],ic=indices[i+2];
                    if(ia<0||ib<0||ic<0||ia>=mesh.Vertices.Count||ib>=mesh.Vertices.Count||ic>=mesh.Vertices.Count) continue;
                    var va=mesh.Vertices[ia];var vb=mesh.Vertices[ib];var vc=mesh.Vertices[ic];
                    var normal=Vector3.Transform(va.Normal+vb.Normal+vc.Normal,spin);
                    if(normal.Y<=0.05f) continue; // floors/terrain, never roofs' undersides or ceilings
                    var a=Vector3.Transform(va.Position,matrix);var b=Vector3.Transform(vb.Position,matrix);var c=Vector3.Transform(vc.Position,matrix);
                    if(!Finite(a)||!Finite(b)||!Finite(c)) throw new InvalidDataException("nonfinite terrain vertex");
                    float area=Math.Abs((b.X-a.X)*(c.Z-a.Z)-(b.Z-a.Z)*(c.X-a.X));
                    if(area<0.03f) continue; // tiny decorative slivers do not define landforms
                    string PointKey(Vector3 p)=>$"{MathF.Round(p.X*4)},{MathF.Round(p.Y*4)},{MathF.Round(p.Z*4)}";
                    var key=new[]{PointKey(a),PointKey(b),PointKey(c)};Array.Sort(key,StringComparer.Ordinal);
                    if(!seen.Add(string.Join(";",key)+";"+kind)) continue;
                    if(faces.Count>=500000) throw new InvalidDataException("terrain record limit");
                    faces.Add((a,b,c,kind));
                }
            }
        }
    }
    private static bool Finite(Vector3 p)=>float.IsFinite(p.X)&&float.IsFinite(p.Y)&&float.IsFinite(p.Z)&&Math.Abs(p.X)<100000&&Math.Abs(p.Y)<100000&&Math.Abs(p.Z)<100000;
    public void Write(string path,byte[] block) {
        using var stream=File.Create(path);using var w=new BinaryWriter(stream);
        w.Write(Encoding.ASCII.GetBytes("ERLAND01"));w.Write(block);w.Write(faces.Count);
        foreach(var f in faces) {foreach(var p in new[]{f.a,f.b,f.c}) {w.Write(p.X);w.Write(p.Y);w.Write(p.Z);} w.Write(f.kind);w.Write(new byte[3]);}
    }
}
