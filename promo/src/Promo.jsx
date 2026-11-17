import {AbsoluteFill, Sequence, interpolate, spring, useCurrentFrame, useVideoConfig, Easing} from 'remotion';

const C = {green: '#4cc26f', grey: '#8c8c8c', track: '#3a3a3a', bg: '#1f1f1f', white: '#f0f0f0', amber: '#ffb900', red: '#ff6361', ink: '#0d1117'};
const FONT = 'Inter, "Segoe UI", sans-serif';
const lvl = (p) => (p >= 90 ? C.red : p >= 75 ? C.amber : null);
const clamp = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'};

const Rings = ({size = 120, outer = 75, inner = 50, oc = C.green, ic = C.white, bg = true}) => {
  const ring = (r, w, p, col) => {
    const c = 2 * Math.PI * r;
    return (
      <>
        <circle cx="60" cy="60" r={r} fill="none" stroke={C.track} strokeWidth={w} />
        <circle cx="60" cy="60" r={r} fill="none" stroke={col} strokeWidth={w} strokeDasharray={`${(c * p) / 100} ${c}`} transform="rotate(-90 60 60)" />
      </>
    );
  };
  return (
    <svg width={size} height={size} viewBox="0 0 120 120">
      {bg && <rect width="120" height="120" rx="26" fill={C.bg} />}
      {ring(40.8, 13.2, outer, oc)}
      {ring(22.8, 12, inner, ic)}
    </svg>
  );
};

const Backdrop = ({children}) => {
  const f = useCurrentFrame();
  const x = 50;
  return (
    <AbsoluteFill style={{background: `radial-gradient(circle at ${x}% 30%, #23456b 0%, #12233a 55%, #0b1422 100%)`, fontFamily: FONT}}>
      {children}
    </AbsoluteFill>
  );
};

const Caption = ({text, sub}) => {
  const f = useCurrentFrame();
  const o = interpolate(f, [0, 10], [0, 1], clamp);
  const y = interpolate(f, [0, 10], [12, 0], clamp);
  return (
    <div style={{position: 'absolute', top: 44, width: '100%', textAlign: 'center', opacity: o, transform: `translateY(${y}px)`}}>
      <div style={{color: '#fff', fontSize: 44, fontWeight: 700, letterSpacing: -0.5}}>{text}</div>
      {sub && <div style={{color: '#9fb6cf', fontSize: 22, marginTop: 8}}>{sub}</div>}
    </div>
  );
};

const Fade = ({dur, children}) => {
  const f = useCurrentFrame();
  const o = interpolate(f, [0, 8, dur - 8, dur], [0, 1, 1, 0], clamp);
  return <AbsoluteFill style={{opacity: o}}>{children}</AbsoluteFill>;
};

const Pill = ({items, live = true, width, scale = 1.6, glow = 0, style}) => (
  <div
    style={{
      background: '#202020',
      color: C.white,
      borderRadius: 10,
      padding: '6px 16px 6px 34px',
      position: 'relative',
      fontWeight: 600,
      fontSize: 15,
      width,
      boxSizing: 'border-box',
      transform: `scale(${scale})`,
      transformOrigin: 'center',
      boxShadow: `0 8px 30px rgba(0,0,0,.45), 0 0 ${18 * glow}px ${6 * glow}px rgba(255,99,97,${0.55 * glow})`,
      ...style,
    }}
  >
    <div style={{position: 'absolute', left: 14, top: 14, width: 9, height: 9, borderRadius: 5, background: live ? C.green : C.grey}} />
    <div style={{display: 'flex', flexWrap: 'wrap', columnGap: 18, rowGap: 2}}>
      {items.map((it, i) => (
        <div key={i} style={{height: 25, display: 'flex', alignItems: 'center', whiteSpace: 'nowrap', opacity: it.o ?? 1, transform: `scale(${it.s ?? 1})`}}>
          {it.label && <span>{it.label}&nbsp;</span>}
          <span style={{color: it.p != null ? lvl(it.p) || C.white : C.white}}>{it.text ?? `${Math.round(it.p)}%`}</span>
        </div>
      ))}
    </div>
  </div>
);

const Taskbar = ({s, w, live = true, clock = '1:42 pm'}) => {
  const oc = live ? lvl(w) || C.green : C.grey;
  const ic = live ? lvl(s) || C.white : C.grey;
  return (
    <div style={{position: 'absolute', bottom: 0, left: 0, right: 0, height: 56, background: 'rgba(20,20,20,.92)', display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 18, paddingRight: 28, color: '#ddd', fontSize: 18}}>
      <span style={{color: '#888'}}>^</span>
      <Rings size={34} outer={w} inner={s} oc={oc} ic={ic} />
      <span>{clock}</span>
    </div>
  );
};

// 1. Title
const Title = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const sp = spring({frame: f, fps, config: {damping: 14}});
  const fill = interpolate(f, [5, 45], [0, 1], {...clamp, easing: Easing.out(Easing.cubic)});
  const t = interpolate(f, [18, 32], [0, 1], clamp);
  return (
    <Backdrop>
      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', flexDirection: 'column'}}>
        <div style={{transform: `scale(${sp})`}}>
          <Rings size={170} outer={75 * fill} inner={50 * fill} />
        </div>
        <div style={{color: '#fff', fontSize: 84, fontWeight: 800, letterSpacing: -2, marginTop: 22, opacity: t, transform: `translateY(${(1 - t) * 20}px)`}}>TokenStat</div>
        <div style={{color: '#9fb6cf', fontSize: 30, marginTop: 6, opacity: interpolate(f, [30, 44], [0, 1], clamp)}}>Know your Claude limits before they find you.</div>
      </AbsoluteFill>
    </Backdrop>
  );
};

// 2. Problem
const CODE = ['fn refactor_auth(ctx: &Context) -> Result<()> {', '    let session = ctx.session()?;', '    migrate_tokens(&session, Policy::Strict)?;', '    rotate_keys(&ctx.vault)?;', '    update_routes(&ctx.router)?;', '    Ok(())', '}'];
const Problem = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const shown = Math.floor(interpolate(f, [8, 50], [0, CODE.length], clamp));
  const hit = 55;
  const drop = spring({frame: f - hit, fps, config: {damping: 9, stiffness: 180}});
  const shake = f > hit && f < hit + 14 ? Math.sin((f - hit) * 2.4) * 10 * (1 - (f - hit) / 14) : 0;
  return (
    <Backdrop>
      <Caption text="Deep in the work..." sub="and then the limit hits. No warning." />
      <div style={{position: 'absolute', left: 290, top: 170, width: 700, height: 330, background: '#161b22', borderRadius: 12, border: '1px solid #30363d', overflow: 'hidden', transform: `translateX(${shake}px)`}}>
        <div style={{height: 34, background: '#0d1117', display: 'flex', alignItems: 'center', gap: 8, paddingLeft: 14}}>
          {['#ff5f56', '#ffbd2e', '#27c93f'].map((c) => <div key={c} style={{width: 12, height: 12, borderRadius: 6, background: c}} />)}
          <span style={{color: '#8b949e', fontSize: 14, marginLeft: 10}}>auth.rs</span>
        </div>
        <div style={{padding: '16px 20px', fontFamily: 'DejaVu Sans Mono, monospace', fontSize: 18, lineHeight: '30px', color: '#c9d1d9'}}>
          {CODE.slice(0, shown).map((l, i) => <div key={i} style={{whiteSpace: 'pre'}}>{l}</div>)}
        </div>
        <div style={{position: 'absolute', left: 20, right: 20, bottom: 20, transform: `translateY(${(1 - drop) * 120}px)`, background: '#3d1414', border: `2px solid ${C.red}`, color: '#ffd7d6', borderRadius: 10, padding: '12px 16px', fontSize: 20, fontWeight: 600, opacity: f > hit ? 1 : 0}}>
          ⚠ Usage limit reached. Your work stops here.
        </div>
      </div>
    </Backdrop>
  );
};

// 3. Solution: colour by usage
const Solution = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const enter = spring({frame: f - 4, fps, config: {damping: 12}});
  const t = interpolate(f, [26, 120], [0, 1], {...clamp, easing: Easing.inOut(Easing.quad)});
  const s = 20 + 76 * t;
  const w = 38 + 54 * t;
  const cpu = 15 + 70 * Math.abs(Math.sin(t * 3.1));
  const ram = 50 + 44 * t;
  const glow = s >= 90 ? 0.6 + 0.4 * Math.sin(f / 4) : 0;
  const callout = s >= 90 ? 'Red at 90%' : s >= 75 || w >= 75 ? 'Amber at 75%' : 'Every figure, colour-coded';
  return (
    <Backdrop>
      <Caption text="TokenStat keeps it on screen" sub="Session, weekly and system stats, coloured by usage" />
      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
        <div style={{transform: `translateY(${(1 - enter) * 260}px)`, opacity: enter, marginTop: 40}}>
          <Pill glow={glow} items={[{label: 'Session', p: s}, {label: 'Weekly', p: w}, {label: 'CPU', p: cpu}, {label: 'RAM', p: ram}]} />
        </div>
        <div style={{position: 'absolute', top: 400, color: s >= 90 ? C.red : s >= 75 || w >= 75 ? C.amber : '#9fb6cf', fontSize: 26, fontWeight: 700}}>{callout}</div>
      </AbsoluteFill>
      <Taskbar s={s} w={w} />
    </Backdrop>
  );
};

// 4. VS Code
const VsCode = () => {
  const f = useCurrentFrame();
  const used = f > 22;
  const refreshing = f > 45 && f < 62;
  const fresh = f >= 62;
  const live = !used || fresh;
  const vals = fresh ? [34, 21] : [21, 19];
  return (
    <Backdrop>
      <Caption text="Lives in VS Code? So does TokenStat." sub="Notices extension use, then refreshes in seconds (opt-in)" />
      <div style={{position: 'absolute', left: 120, top: 190, width: 440, height: 250, background: '#1e1e1e', borderRadius: 10, border: '1px solid #333', padding: 16, boxSizing: 'border-box', color: '#ccc', fontSize: 17}}>
        <div style={{color: '#888', fontSize: 14, marginBottom: 12}}>VS Code · Claude Code extension</div>
        <div style={{background: '#2d2d30', borderRadius: 6, padding: '8px 10px', marginBottom: 8}}>Refactor the auth module</div>
        {used && <div style={{background: '#2d2d30', borderRadius: 6, padding: '8px 10px'}}>Done. 14 files changed.</div>}
      </div>
      <div style={{position: 'absolute', left: 700, top: 270}}>
        <Pill scale={1.5} live={live} items={[{label: 'Session', p: vals[0]}, {label: 'Weekly', p: vals[1]}]} />
        <div style={{marginTop: 34, fontSize: 22, fontWeight: 600, color: fresh ? C.green : used ? '#c7c7c7' : '#9fb6cf', textAlign: 'center', width: 330}}>
          {fresh ? '✓ Fresh figures' : refreshing ? '↻ Refreshing…' : used ? 'Used since. Reading is old' : 'Current reading'}
        </div>
      </div>
      <Taskbar s={vals[0]} w={vals[1]} live={live} clock="10:21 am" />
    </Backdrop>
  );
};

// 5. Shape
const Shape = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const pop = (start) => spring({frame: f - start, fps, config: {damping: 16, stiffness: 180}});
  const stats = [
    {label: 'CPU', p: 78},
    {label: 'RAM', p: 93},
    {label: 'GPU', p: 6},
    {label: null, text: '↓1.2 MB/s ↑340 KB/s'},
  ];
  const items = [{label: 'Session', p: 34}, {label: 'Weekly', p: 21}, ...stats.map((s, i) => ({...s, s: pop(8 + i * 7), o: Math.min(1, pop(8 + i * 7))}))];
  const width = interpolate(f, [40, 62], [760, 270], {...clamp, easing: Easing.inOut(Easing.cubic)});
  const hideLabels = f > 78;
  return (
    <Backdrop>
      <Caption text="Your shape. Your stats." sub="Add CPU, RAM, GPU, network. Drag the edge: one line or a stack." />
      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
        <div style={{marginTop: 60}}>
          <Pill scale={1.35} width={width} items={items.map((it) => (hideLabels && it.label ? {...it, label: null} : it))} />
        </div>
      </AbsoluteFill>
    </Backdrop>
  );
};

// 6. End card
const End = () => {
  const f = useCurrentFrame();
  const {fps} = useVideoConfig();
  const chips = ['Local only', 'No telemetry', 'Native Rust', 'MIT licensed'];
  return (
    <Backdrop>
      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', flexDirection: 'column'}}>
        <Rings size={110} outer={interpolate(f, [0, 30], [0, 75], clamp)} inner={interpolate(f, [0, 30], [0, 50], clamp)} />
        <div style={{color: '#fff', fontSize: 64, fontWeight: 800, letterSpacing: -1.5, marginTop: 14}}>TokenStat</div>
        <div style={{display: 'flex', gap: 14, marginTop: 22}}>
          {chips.map((c, i) => {
            const s = spring({frame: f - 10 - i * 5, fps, config: {damping: 12}});
            return <div key={c} style={{transform: `scale(${s})`, background: 'rgba(76,194,111,.15)', border: `1px solid ${C.green}`, color: '#c9f0d5', borderRadius: 999, padding: '8px 18px', fontSize: 20, fontWeight: 600}}>{c}</div>;
          })}
        </div>
        <div style={{color: '#9fb6cf', fontSize: 24, marginTop: 26, opacity: interpolate(f, [30, 42], [0, 1], clamp)}}>Free and open source for Windows 11</div>
        <div style={{color: '#5f7690', fontSize: 15, marginTop: 30}}>Independent project. Not affiliated with Anthropic.</div>
      </AbsoluteFill>
    </Backdrop>
  );
};

const SCENES = [
  [Title, 0, 70],
  [Problem, 70, 95],
  [Solution, 165, 150],
  [VsCode, 315, 90],
  [Shape, 405, 105],
  [End, 510, 70],
];

export const Promo = () => (
  <AbsoluteFill style={{background: '#0b1422'}}>
    {SCENES.map(([S, from, dur], i) => (
      <Sequence key={i} from={from} durationInFrames={dur}>
        <Fade dur={dur}><S /></Fade>
      </Sequence>
    ))}
  </AbsoluteFill>
);
