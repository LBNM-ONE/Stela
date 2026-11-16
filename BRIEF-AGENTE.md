# Brief per l'agente delle fasi B, C e D

Questo file dice a un agente di codice come continuare il porting di Stela
alla AMD Radeon RX 9070 dell'utente. Lo ha scritto Claude, l'agente che ha
fatto il lavoro fin qui, il 2026-11-16. Claude è il **supervisore** di questo
lavoro: quando l'utente tornerà da lui, rivedrà ogni commit dell'agente prima
di unirlo al branch principale. Lavora in modo che quella revisione sia
facile.

## Il contesto in breve

- Repository: fork `LBNM-ONE/Stela`, cartella `/home/ubuntu/projects/Stela`,
  sul PC dell'utente (Ubuntu, Ryzen 9 7900X con iGPU Raphael, RX 9070 Navi
  48 su `03:00.0`, scheda madre Gigabyte B850). Lavori sullo stesso PC su
  cui l'utente fa le prove.
- L'utente parla italiano. Scrivigli in italiano.
- Stato al 2026-11-16: amdgpu prende la RX 9070 dentro Stela (firmware,
  SMU, 56 CU), macOS è installato sul disco interno e si avvia con amdgpu
  sulla 9070, ma senza device Metal: WindowServer non parte. Per avere
  Metal sulla 9070 servono RADV (il driver Vulkan di Mesa per AMD) e il
  device Metal di Stela sopra RADV.
- Tutto è documentato in `HANDOFF.md` (in italiano), la memoria del
  progetto. Leggilo per intero prima di scrivere codice, in particolare:
  «Come Stela gestisce NVIDIA oggi», «Fatti su RADV nel Mesa pinnato»,
  «Cosa manca, per strato», «Piano a tappe proposto» (le fasi A-D),
  «Ambiente di build sul PC», «Prova sul PC: risultati» e «Stato».
- La fase A è fatta: Mesa con NVK e LLVM compilano su questo PC, e
  l'immagine con NVK e il device Metal linka. Tocca a te la B, poi la C,
  poi la D.

## Prima di cominciare

```sh
cd /home/ubuntu/projects/Stela
export PATH="$HOME/.rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH"
export CLANG="$HOME/.local/share/swiftly/toolchains/6.4.1/usr/bin/clang"
git switch claude/amd-radeon-rx9070-rdna4-y12vd2
git switch -c radv-fase-b
```

Il `PATH` serve perché `cargo` nel PATH di sistema è un 1.93 di Ubuntu, il
repository vuole il 1.99.1. `CLANG` serve perché il Clang di questo PC viene
da swiftly, i cui proxy si rifiutano di chiamarsi a vicenda (vedi HANDOFF).
Tutti i tuoi commit vanno sul branch `radv-fase-b`.

## Regole, non negoziabili

1. **Git**: niente push, niente merge nel branch principale, niente
   rebase, amend o force su commit che non sono tuoi, non toccare `main`.
   Il supervisore unirà il tuo branch dopo averlo rivisto.
2. **Niente sudo**: chiede la password. Se un comando richiede root, chiedi
   all'utente di lanciarlo e di incollarti l'output.
3. **Niente fuori dal repository**: non toccare la chiavetta USB, le
   partizioni dei dischi, il BIOS né file fuori da
   `/home/ubuntu/projects/Stela`. Se serve spazio cancella solo output di
   build in `target/`, mai `git clean -fdx`, che toglierebbe anche `.tools`.
4. **Niente upstream**: l'utente ha deciso di non contattare
   `tylertms/Stela`. Niente issue, PR o messaggi ad altri.
5. **Onestà**: non scrivere mai che una cosa è verificata se non l'hai
   verificata con un comando. Se una cosa non ti riesce, dillo e scrivilo
   in HANDOFF.
6. **Riavvii**: il PC delle prove è quello su cui giri. Quando l'utente
   riavvia per una prova la tua sessione si ferma, quindi prima di ogni
   riavvio fai commit e aggiorna HANDOFF.
7. **Stile del repository**: codice e commenti in inglese, con la stessa
   densità di commenti del codice intorno; `unsafe` sempre con il suo
   commento `// SAFETY:`; niente `#[allow]` o test disattivati per far
   passare i controlli. Commit piccoli, uno per passo logico, ciascuno
   compilato e verificato. Messaggi di commit nello stile del repository:
   `tipo(ambito): una frase lunga che dice cosa e perché`, riga vuota, un
   corpo che spiega il perché, un paragrafo `Checked:` con i comandi
   eseguiti e i risultati, e la tua riga di attribuzione in fondo.
8. **HANDOFF.md** resta in italiano: aggiorna «Stato» a ogni passo, con
   data, commit, cosa è verificato e come, cosa no.
9. **Patch a Mesa o Linux**: mai modificare `third_party/` a mano. I
   submodule sono sul branch `stela` con la serie di patch applicata: fai
   un commit nel submodule (per Mesa, `third_party/mesa`), poi
   `cargo xtask gpu export` lo scrive in `third_party/patches/mesa/`, e
   `cargo xtask gpu fetch` lo riapplica.
10. **Non rompere quello che c'è**: alla fine di ogni passo devono ancora
    funzionare la lane NVK, l'immagine con amdgpu, i test e clippy (vedi
    «Verifiche standard»).

## Fase B: la lane RADV

Obiettivo: un archivio `target/gpu/radv/libstela-radv.a` con RADV compilato
per `stela.efi`, e un'immagine con amdgpu e RADV che linka.

1. **La lane in xtask.** Aggiungi `cargo xtask gpu radv`, gemella di
   `cargo xtask gpu nvk` (`xtask/src/gpu/nvk.rs`; le opzioni di Meson sono
   in `OPTIONS`, con `-Dvulkan-drivers=nouveau`). Preferisci parametrizzare
   la lane esistente piuttosto che duplicarla, senza cambiare cosa produce
   per NVK. Tieni conto di `xtask/src/gpu/toolchain.rs`: ogni opzione o
   flag nuovo va nell'identità che fa ricostruire gli alberi di build.
2. **ACO** è in C++: compilalo contro la libc++ che la lane LLVM costruisce
   per `stela.efi` (`xtask/src/gpu/llvm.rs`). NVK usa `-Dcpp_rtti=false`:
   controlla cosa vuole ACO per RTTI ed eccezioni.
3. **`libdrm_amdgpu`.** RADV chiama 17 funzioni di `libdrm_amdgpu`, elencate
   in HANDOFF («Fatti su RADV nel Mesa pinnato»). Oggi `third_party/libdrm`
   ha solo gli header (`third_party/libdrm.sparse`). Le strade sono due:
   allargare lo sparse a `/amdgpu/*.c` e compilarli sopra il compat DRM di
   Stela (`crates/stela-driver-gpu/src/compat/mesa/drm.c`), oppure scriverle
   in `crates/stela-driver-gpu/src/compat/mesa/` seguendo libdrm (licenza
   MIT, da citare in `NOTICE`). Valuta, scegli e scrivi il perché in
   HANDOFF. Le ioctl arrivano all'amdgpu dell'ambiente Linux di Stela.
4. **Gli shader NIR.** Il device Metal passa gli shader come NIR in
   `VkPipelineShaderStageNirCreateInfoMESA`
   (`crates/stela-driver-gpu/src/metal/native.rs`). RADV accetta NIR solo
   tramite `vk_shader_module.nir`. Serve una patch Mesa piccola (regola 9)
   perché RADV prenda il NIR da quella struttura, come fa NVK.
5. **Il link.** Una feature `radv` in `crates/stela-boot` (che implica
   `amdgpu`) e un'opzione di `cargo xtask boot build` (per esempio
   `--radv`, accanto a `--amdgpu` in `xtask/src/boot.rs`) che linka
   `libstela-radv.a` come la feature `nvk` linka `libstela-nvk.a`.
   L'immagine deve linkare.
6. **Le patch di NVK.** 20 delle 26 patch in `third_party/patches/mesa/`
   toccano `src/nouveau` (interlock, scratch degli shader, bind VA
   asincroni, rifiuti di submit). Per ognuna scrivi in HANDOFF se a RADV
   serve un equivalente, e perché. In questa fase non portarle.

Fine fase B: commit, HANDOFF aggiornato, un riassunto all'utente.

## Fase C: il primo comando sulla 9070

Obiettivo: provare tutta la catena RADV → amdgpu → GPU sulla 9070, ancora
senza Metal.

1. Nel GPU check (`--gpu-check`, in `crates/stela-boot/src/engine/start.rs`,
   righe con `GPU CHECK`), dopo che amdgpu ha preso la GPU: prendi il
   `vkGetInstanceProcAddr` di RADV, crea l'instance, scegli la 9070, crea
   device e coda, alloca un buffer, registra un `vkCmdFillBuffer` con un
   valore noto, fai submit, aspetta, mappa il buffer e controlla il valore.
   Ogni passo scrive una riga `RADV …` nel log, e alla fine
   `GPU CHECK radv=ok` oppure `GPU CHECK radv=failed: <motivo>`.
2. L'immagine si costruisce con l'opzione della fase B più `--amdgpu
   --diagnostics`.
3. La prova la fa l'utente. La procedura completa è in HANDOFF:
   - con la chiavetta montata in `/run/media/ubuntu/EFI` (puoi montarla tu
     con `udisksctl mount -b /dev/sda1`), l'utente lancia
     `sudo target/release/stela update /run/media/ubuntu/EFI --efi target/boot/x86_64-unknown-uefi/release/stela-deploy.efi`;
   - poi tu aggiungi a `EFI/stela/choices.cfg` la voce «GPU check»,
     copiando la riga di «Install macOS» con ` --gpu-check` in fondo (un tab
     separa nome e opzioni), e smonti la chiavetta;
   - l'utente lascia la LAN disattivata nel BIOS, avvia con F12 dalla
     partizione 1 e sceglie «GPU check». I tasti del menu contano solo le
     voci mostrate, e con macOS installato il menu avvia la prima voce dopo
     5 secondi;
   - al ritorno su Ubuntu, monti la chiavetta in sola lettura
     (`udisksctl mount -b /dev/sda1 -o ro`), copi `EFI/stela/boot.log` e ne
     estrai il testo con `strings`. `stela log` chiede sudo. Il log è un
     anello da 4 MiB con più avvii: cerca l'ultimo `STELA UEFI MOUNT`.

Fine fase C: HANDOFF aggiornato e riassunto all'utente.

## Fase D: il device Metal su RADV

1. `crates/stela-driver-gpu/src/metal/amd.rs` accanto a `metal/nvidia.rs`
   (ammissione del device, `Platform` con allineamenti e pagina sparse). Il
   backend `metal/native.rs` accetta già un driver Mesa qualsiasi tramite
   `Loader`: `get_instance_proc_addr`, mentre `submit_thread` e
   `refusals`, propri di NVK, sono opzionali.
2. `crates/stela-boot/src/engine/gpu/gate.rs` (dove passa
   `get_instance_proc_addr: entry`) e `gpu::no_metal_device` in
   `crates/stela-boot/src/engine/gpu.rs`: con `radv` la 9070 deve dare a
   macOS il suo device Metal.
3. Il modello della GPU Apple (AGX) va nascosto a macOS quando c'è il device
   Metal, come fa NVK. Il commit `2eb5cf7cd` lo nasconde nelle immagini
   senza NVK: adattalo a `radv`.
4. La suite Metal (`cargo xtask gpu metal-suite`), poi un avvio di macOS
   normale, con la prova dell'utente come nella fase C.

## Verifiche standard

Prima di ogni commit, sulle parti che hai toccato:

```sh
cargo fmt --all --check
cargo test --locked -p stela-driver-gpu -p stela-linux --all-features
cargo test --locked -p <gli altri crate che hai toccato> --lib
BF='--cfg aes_force_soft --cfg chacha20_backend="soft" --cfg aes_backend="soft" --cfg polyval_backend="soft" --cfg poly1305_backend="soft" -D warnings'
U="--locked --target x86_64-unknown-uefi --target-dir target/boot"
RUSTFLAGS="$BF" cargo clippy --manifest-path crates/stela-boot/Cargo.toml --features amdgpu,diagnostics $U -- -D warnings
RUSTFLAGS="$BF" cargo clippy --manifest-path crates/stela-boot/Cargo.toml --features diagnostics $U -- -D warnings
RUSTFLAGS="$BF" cargo clippy --manifest-path crates/stela-boot/Cargo.toml $U -- -D warnings
cargo xtask gpu nvk --jobs 16
cargo xtask boot build --release --amdgpu --diagnostics
```

Clippy non vede il C: dopo modifiche al C compila anche
`cargo build --locked -p stela-driver-gpu --features amdgpu-port --target x86_64-unknown-uefi --target-dir target/boot`.
Aggiungi i controlli della lane RADV quando esiste. Riporta nei commit cosa
hai eseguito e il risultato.

## Come chiudere

- A ogni fine fase: «Stato» in HANDOFF con data, commit e verifiche, e lo
  stesso riassunto all'utente in italiano.
- Se ti blocchi: scrivi in HANDOFF il punto esatto, cosa hai provato e
  l'errore, poi fermati e chiedi all'utente.
- Alla fine lascia il branch `radv-fase-b` pulito: `git status` senza
  modifiche, ogni passo nel suo commit. Il supervisore lo rivedrà prima di
  unirlo.
