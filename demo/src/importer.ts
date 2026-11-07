import { uid, type Footage, type Project } from './engine/model';
import { toast } from '../../frontend/src/ui/toast';
import { addFootageLayer, bus, comp, compFromFootage, loadProject, S } from './state';

const IMG = /\.(png|jpe?g|gif|webp|bmp|avif|svg)$/i;
const VID = /\.(mp4|m4v|mov|webm|ogv|mkv)$/i;

function readDataUrl(f: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result));
    r.onerror = () => rej(r.error);
    r.readAsDataURL(f);
  });
}

async function importImage(f: File): Promise<Footage> {
  const data = await readDataUrl(f);
  const img = new Image();
  img.src = data;
  await img.decode();
  return { id: uid('F'), name: f.name, kind: 'image', w: img.naturalWidth || 1, h: img.naturalHeight || 1, dur: 0, gen: null, data, size: f.size, mime: f.type || 'image', src: img };
}

async function importVideo(f: File): Promise<Footage> {
  const url = URL.createObjectURL(f);
  const v = document.createElement('video');
  v.muted = true; v.playsInline = true; v.preload = 'auto'; v.src = url;
  await new Promise<void>((res, rej) => {
    v.onloadeddata = () => res();
    v.onerror = () => rej(new Error(`${f.name} cannot be decoded by this browser`));
  });
  return { id: uid('F'), name: f.name, kind: 'video', w: v.videoWidth || 1280, h: v.videoHeight || 720, dur: Number.isFinite(v.duration) ? v.duration : 0, gen: null, data: null, size: f.size, mime: f.type || 'video', src: v, url };
}

export async function importFiles(files: File[], asComp = false): Promise<void> {
  let added = 0;
  for (const f of files) {
    try {
      if (/\.(json|fvxdemo)$/i.test(f.name)) {
        const p = JSON.parse(await f.text()) as Project;
        if (!Array.isArray(p.comps) || !Array.isArray(p.footage)) throw new Error('not a FormVFX demo project');
        await Promise.all(p.footage.filter((x) => x.kind === 'image' && x.data).map(async (x) => {
          const img = new Image();
          img.src = x.data as string;
          await img.decode();
          x.src = img;
        }));
        loadProject(p, f.name);
        toast(`Opened ${f.name}`);
        added += 1;
        continue;
      }
      let foot: Footage;
      if (f.type.startsWith('image/') || IMG.test(f.name)) foot = await importImage(f);
      else if (f.type.startsWith('video/') || VID.test(f.name)) foot = await importVideo(f);
      else { toast(`${f.name}: audio and other types are not part of this demo`, 'error'); continue; }
      S.project.footage.push(foot);
      if (asComp || !S.project.comps.length) compFromFootage(foot);
      else addFootageLayer(foot);
      added += 1;
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), 'error');
    }
  }
  if (added) { bus.emit('project'); bus.emit('comp'); if (comp()) bus.emit('frame'); }
}

export function pickFiles(accept: string, multiple: boolean): Promise<File[]> {
  return new Promise((res) => {
    const i = document.createElement('input');
    i.type = 'file'; i.accept = accept; i.multiple = multiple;
    i.onchange = () => res(Array.from(i.files ?? []));
    i.click();
  });
}
