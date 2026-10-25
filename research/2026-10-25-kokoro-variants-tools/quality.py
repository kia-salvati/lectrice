"""Compare a variant's WAVs with the fp32 reference: duration ratio, MCD (DTW-aligned, 24 mel-cepstra), log-mel L1, peak, clipping."""
import sys, glob, os, wave, numpy as np
SR = 24000; NFFT = 1024; HOP = 256; NMEL = 80; NCEP = 25
def read(p):
    import struct
    with open(p, "rb") as f: data = f.read()
    # hound float32 WAV: find 'data' chunk
    i = data.find(b"data"); n = struct.unpack("<I", data[i+4:i+8])[0]
    return np.frombuffer(data[i+8:i+8+n], dtype="<f4").astype(np.float64)
def melfb():
    hz = lambda m: 700*(10**(m/2595)-1); mel = lambda h: 2595*np.log10(1+h/700)
    pts = hz(np.linspace(mel(0), mel(SR/2), NMEL+2)); bins = np.floor((NFFT+1)*pts/SR).astype(int)
    fb = np.zeros((NMEL, NFFT//2+1))
    for m in range(1, NMEL+1):
        l, c, r = bins[m-1], bins[m], bins[m+1]
        fb[m-1, l:c] = (np.arange(l, c)-l)/max(c-l, 1); fb[m-1, c:r] = (r-np.arange(c, r))/max(r-c, 1)
    return fb
FB = melfb(); WIN = np.hanning(NFFT)
DCT = np.sqrt(2/NMEL)*np.cos(np.pi/NMEL*(np.arange(NMEL)+0.5)[None, :]*np.arange(NCEP)[:, None])
def feats(x):
    n = 1 + max(0, (len(x)-NFFT)//HOP)
    fr = np.stack([x[i*HOP:i*HOP+NFFT]*WIN for i in range(n)])
    pw = np.abs(np.fft.rfft(fr, axis=1))**2
    mel = FB @ pw.T
    lm = 0.5*np.log(np.maximum(mel, mel.max()*1e-8)).T  # ln amplitude, floor 80 dB below the loudest bin
    e = lm.max(axis=1); keep = e > e.max() - 4.6  # drop near-silent frames (~ -40 dB)
    return lm, (lm @ DCT.T)[:, 1:], keep
def dtw(a, b):
    d = np.sqrt(((a[:, None, :]-b[None, :, :])**2).sum(-1))
    n, m = d.shape; D = np.full((n+1, m+1), np.inf); D[0, 0] = 0
    for i in range(1, n+1):
        row = D[i]; prev = D[i-1]; di = d[i-1]
        for j in range(1, m+1):
            row[j] = di[j-1] + min(prev[j], prev[j-1], row[j-1])
    i, j, path = n, m, []
    while i > 0 and j > 0:
        path.append((i-1, j-1)); k = np.argmin([D[i-1, j-1], D[i-1, j], D[i, j-1]])
        if k == 0: i, j = i-1, j-1
        elif k == 1: i -= 1
        else: j -= 1
    return path[::-1]
def compare(ref, var):
    lr, cr, kr = feats(ref); lv, cv, kv = feats(var)
    path = dtw(cr, cv)
    path = [(i, j) for i, j in path if kr[i]]
    diff = np.array([cr[i]-cv[j] for i, j in path])
    mcd = (10/np.log(10))*np.sqrt(2*(diff**2).sum(1)).mean()
    l1 = np.mean([np.abs(lr[i]-lv[j]).mean() for i, j in path])
    return mcd, l1
if __name__ == "__main__":
    ref_dir, var_dir = sys.argv[1], sys.argv[2]
    rows = []
    for p in sorted(glob.glob(f"{ref_dir}/s*.wav")):
        q = os.path.join(var_dir, os.path.basename(p))
        r, v = read(p), read(q)
        mcd, l1 = compare(r, v)
        rows.append((len(v)/len(r), mcd, l1, np.abs(v).max(), int((np.abs(v) >= 0.999).sum()), int((~np.isfinite(v)).sum())))
    a = np.array(rows)
    dev = np.abs(a[:, 0]-1)*100
    print(f"{os.path.basename(var_dir):14s} dur_dev_max {dev.max():5.2f}% mean {dev.mean():4.2f}%  MCD {a[:,1].mean():5.2f} dB (max {a[:,1].max():5.2f})  logmelL1 {a[:,2].mean():5.3f}  peak {a[:,3].max():.3f}  clipped {int(a[:,4].sum())} nonfinite {int(a[:,5].sum())}")
