import math, sys

def squircle(cx, cy, size, n=3.0, pts=90):
    e = 0.30*size; h = size/2
    corners = [(cx+h-e, cy-h+e, 0), (cx+h-e, cy+h-e, 90), (cx-h+e, cy+h-e, 180), (cx-h+e, cy-h+e, 270)]
    out=[]
    for (ox, oy, start) in corners:
        for i in range(pts+1):
            t = math.radians(start - 90 + 90*i/pts)
            c, s_ = math.cos(t), math.sin(t)
            out.append(f"{ox+e*math.copysign(abs(c)**(2/n),c):.2f},{oy+e*math.copysign(abs(s_)**(2/n),s_):.2f}")
    return "M"+" L".join(out)+" Z"

def rdiamond(cx, cy, r, rr):
    # diamond with rounded corners, half-diagonal r, corner radius rr
    pts=[(cx,cy-r),(cx+r,cy),(cx,cy+r),(cx-r,cy)]
    out=[]
    for i in range(4):
        p0,p1,p2=pts[i-1],pts[i],pts[(i+1)%4]
        def tw(a,b,d):
            L=math.hypot(b[0]-a[0],b[1]-a[1]); return (a[0]+(b[0]-a[0])*d/L, a[1]+(b[1]-a[1])*d/L)
        out.append((tw(p1,p0,rr),p1,tw(p1,p2,rr)))
    d=f"M{out[0][2][0]:.1f},{out[0][2][1]:.1f} "
    for i in range(1,5):
        a,p,b=out[i%4]; d+=f"L{a[0]:.1f},{a[1]:.1f} Q{p[0]:.1f},{p[1]:.1f} {b[0]:.1f},{b[1]:.1f} "
    return d+"Z"

def bez(t, p0, p1, p2):
    return ((1-t)**2*p0[0]+2*(1-t)*t*p1[0]+t*t*p2[0], (1-t)**2*p0[1]+2*(1-t)*t*p1[1]+t*t*p2[1])

GX = (-19.7, 17.5, 1.06)
def svg(mode="mac", P=""):
    T = 824 if mode=="mac" else 1024
    k = 1.0 if mode=="mac" else 1.08
    tile = squircle(512,512,T)
    S = lambda x, y: (512+(x-512)*k, 512+(y-512)*k)
    # motion path, bottom left to the keyframe
    p0, p1, p2 = S(222,720), S(330,500), S(590,470)
    main = p2; R = 176*k
    ring = lambda c, r: rdiamond(c[0], c[1], r, r*0.20) + " " + rdiamond(c[0], c[1], r*0.44, r*0.44*0.18)
    ghosts = [(0.58, 0.68, 0.36), (0.30, 0.50, 0.20), (0.08, 0.36, 0.10)]
    g=[f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
<defs>
 <linearGradient id="{P}bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#24212B"/><stop offset="1" stop-color="#0B0A0E"/></linearGradient>
 <radialGradient id="{P}bloom" cx="0.55" cy="0.47" r="0.5"><stop offset="0" stop-color="#FF4F7A" stop-opacity="0.28"/><stop offset="0.25" stop-color="#C255D9" stop-opacity="0.15"/><stop offset="0.5" stop-color="#9A5CFF" stop-opacity="0.07"/><stop offset="1" stop-color="#9A5CFF" stop-opacity="0"/></radialGradient>
 <linearGradient id="{P}rim" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.22"/><stop offset="0.3" stop-color="#fff" stop-opacity="0.04"/><stop offset="1" stop-color="#fff" stop-opacity="0.08"/></linearGradient>
 <linearGradient id="{P}paint" gradientUnits="userSpaceOnUse" x1="{main[0]-R:.0f}" y1="{main[1]+R:.0f}" x2="{main[0]+R:.0f}" y2="{main[1]-R:.0f}"><stop offset="0" stop-color="#3A7BFF"/><stop offset="0.32" stop-color="#9A5CFF"/><stop offset="0.64" stop-color="#FF4F7A"/><stop offset="1" stop-color="#FFAE2B"/></linearGradient>
 <linearGradient id="{P}side" gradientUnits="userSpaceOnUse" x1="{main[0]-R:.0f}" y1="{main[1]+R:.0f}" x2="{main[0]+R:.0f}" y2="{main[1]-R:.0f}"><stop offset="0" stop-color="#1F3F8A"/><stop offset="0.32" stop-color="#4E2D8C"/><stop offset="0.64" stop-color="#8A2443"/><stop offset="1" stop-color="#8A5A14"/></linearGradient>
 <linearGradient id="{P}ghost" gradientUnits="userSpaceOnUse" x1="{p0[0]:.0f}" y1="{p0[1]:.0f}" x2="{main[0]:.0f}" y2="{main[1]:.0f}"><stop offset="0" stop-color="#3A7BFF"/><stop offset="1" stop-color="#9A5CFF"/></linearGradient>
 <linearGradient id="{P}trail" gradientUnits="userSpaceOnUse" x1="{p0[0]:.0f}" y1="{p0[1]:.0f}" x2="{main[0]:.0f}" y2="{main[1]:.0f}"><stop offset="0" stop-color="#3A7BFF" stop-opacity="0"/><stop offset="0.7" stop-color="#9A5CFF" stop-opacity="0.55"/><stop offset="1" stop-color="#FF4F7A" stop-opacity="0.8"/></linearGradient>
 <linearGradient id="{P}sheen" gradientUnits="userSpaceOnUse" x1="0" y1="{main[1]-R:.0f}" x2="0" y2="{main[1]+R:.0f}"><stop offset="0" stop-color="#fff" stop-opacity="0.34"/><stop offset="0.5" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.12"/></linearGradient>
 <filter id="{P}tileshadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity="0.5"/></filter>
 <filter id="{P}markshadow" x="-30%" y="-30%" width="160%" height="170%"><feDropShadow dx="0" dy="{16*k:.1f}" stdDeviation="{20*k:.1f}" flood-color="#000" flood-opacity="0.55"/></filter>
 <filter id="{P}mblur" x="-40%" y="-40%" width="180%" height="180%"><feGaussianBlur stdDeviation="{7*k:.1f} {3*k:.1f}"/></filter>
 <filter id="{P}hshadow" x="-40%" y="-40%" width="180%" height="180%"><feDropShadow dx="0" dy="{5*k:.1f}" stdDeviation="{5*k:.1f}" flood-color="#06020A" flood-opacity="0.6"/></filter>
 <clipPath id="{P}tclip"><path d="{tile}"/></clipPath>
</defs>''']
    if mode=="mac": g.append(f'<path d="{tile}" fill="#000" filter="url(#{P}tileshadow)"/>')
    g.append(f'<path d="{tile}" fill="url(#{P}bg)"/>')
    g.append(f'<g clip-path="url(#{P}tclip)"><rect width="1024" height="1024" fill="url(#{P}bloom)"/></g>')
    dx, dy, sc = GX
    g.append(f'<g transform="translate({512+dx*k:.1f} {512+dy*k:.1f}) scale({sc:.4f}) translate(-512 -512)">')
    # motion path
    path = f"M{p0[0]:.1f},{p0[1]:.1f} Q{p1[0]:.1f},{p1[1]:.1f} {main[0]:.1f},{main[1]:.1f}"
    g.append(f'<path d="{path}" fill="none" stroke="url(#{P}trail)" stroke-width="{6*k:.1f}" stroke-linecap="round" stroke-dasharray="{2*k:.1f} {22*k:.1f}"/>')
    # ghost frames, motion blurred, fading back along the path
    for t, sc, op in ghosts:
        c = bez(t, p0, p1, p2)
        g.append(f'<path d="{ring(c, R*sc)}" fill-rule="evenodd" fill="url(#{P}ghost)" opacity="{op}" filter="url(#{P}mblur)"/>')
    # the keyframe: glossy hollow diamond slab
    d = ring(main, R); dep = 16*k
    g.append(f'<path d="{d}" fill-rule="evenodd" fill="url(#{P}side)" transform="translate(0 {dep:.1f})" filter="url(#{P}markshadow)"/>')
    g.append(f'<path d="{d}" fill-rule="evenodd" fill="url(#{P}paint)"/><path d="{d}" fill-rule="evenodd" fill="url(#{P}sheen)"/>')
    g.append(f'<path d="{rdiamond(main[0], main[1], R, R*0.20)}" fill="none" stroke="#fff" stroke-opacity=".40" stroke-width="{3*k:.1f}"/>')
    # Bezier handle through the keyframe, as in the Graph Editor
    ang = math.radians(-24); L = R*1.32
    a = (main[0]-L*math.cos(ang), main[1]-L*math.sin(ang)); b = (main[0]+L*math.cos(ang), main[1]+L*math.sin(ang))
    g.append(f'<g filter="url(#{P}hshadow)"><path d="M{a[0]:.1f},{a[1]:.1f} L{b[0]:.1f},{b[1]:.1f}" stroke="#fff" stroke-opacity=".85" stroke-width="{6*k:.1f}" stroke-linecap="round"/>'
             f'<circle cx="{a[0]:.1f}" cy="{a[1]:.1f}" r="{17*k:.1f}" fill="#fff"/><circle cx="{b[0]:.1f}" cy="{b[1]:.1f}" r="{17*k:.1f}" fill="#fff"/>'
             f'<circle cx="{main[0]:.1f}" cy="{main[1]:.1f}" r="{13*k:.1f}" fill="#fff"/></g>')
    g.append('</g>')
    g.append(f'<path d="{tile}" fill="none" stroke="url(#{P}rim)" stroke-width="{3*(T/824):.1f}" clip-path="url(#{P}tclip)"/>')
    g.append('</svg>')
    return "\n".join(g)

if __name__=="__main__":
    open(sys.argv[2],"w").write(svg(sys.argv[1]))
