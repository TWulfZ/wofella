# wolluf: entrenador 7K para osu!mania stable (investigación y plan del MVP)

## Contexto

La app es un companion para osu!mania **stable** pensado para cualquier jugador, en la línea de Companella y osu!trainer. El MVP cubre **7K rice + LN** con keymode genérico para poder añadir 4K después. El stack es Tauri 2 (Rust + React/TS).

Objetivo: sacar a los jugadores del estancamiento en patrones concretos. Para eso la app:
1. diagnostica debilidades **por patrón** a partir de los replays;
2. mantiene un **tracker interno de skill por sesión** (qué mejoró, qué empeoró, qué practicar);
3. recomienda mapa, rate y sección exacta;
4. **corta mapas en drills** del patrón débil.

Usuario piloto: TWulfZ (osu! id 23249551). Toca con el pulgar derecho (cols 1-3 mano izquierda | col 4 pulgar derecho + cols 5-7 mano derecha). Está en pre-9th Regular y pre-10th LN, y su meta es Gamma en ambos.

Hubo 3 workflows de investigación (29 agentes) con verificación adversarial. Los scripts y datos están en el scratchpad de la sesión (`audit/`, `rejudge/`, `mania-hub/`).

## Hallazgos que condicionan el diseño

### Competencia y prior art

1. **Companella** (C#, MIT según su LICENSE aunque el README lo contradice, Windows, 4K) ya ofrece MSD, sesiones, estimación de dan y un recomendador.
   - El recomendador es una heurística de bandas de MSD con `KeyCount = 4` fijo.
   - No hace diagnóstico por patrón a partir de replays ni recomendaciones para 7K.
   - Su Marathon Creator borra **todas** las SV cuando hay dos o más líneas rojas. No hay que copiar eso.
2. **mania-hub / mania-tracker.com** tiene el backend cerrado. Solo `algorithms/` es MIT; el resto está sin licencia, así que solo sirve como fuente de ideas.
   - Calcula la skill al estilo Etterna: la precisión estima una meta Wife3, MinaCalc da el SSR y luego se aplica AggregateSSRs.
   - Sus "ratings por patrón" en 7K son el Overall agregado sobre charts etiquetados, **no una skill real por patrón**.
   - Los dans 7K salen de la SR de Sunny pasada por las tablas de intervalos de LeoBlack (MIT). Por ejemplo, Regular Gamma = Sunny 8.537–8.99, con sub-bandas no uniformes.
   - **No entrena nada contra resultados de jugadores**: no usa IRT, Elo ni Glicko.
   - Para 7K LN usa una heurística (Overall + 30 % de colas tratadas como taps).
   - No tiene cortes de práctica ni un tracker de sesión.
   - Ahora mismo clasifica a TWulfZ como 7K rice **9--** y LN **9**, pero su modelo ha cambiado 23 veces desde el 2026-09-03.
   - Lo que conviene tomar: el ajuste NNLS de conteos de judgements a Wife3 (para plays sin replay), las reglas de higiene de datos (excluir HO/NR, vibro y rate stacking) y el protocolo de validación pre-registrada.
3. **Interlude eliminó sus ratings por patrón** (0.7.28.3). Fallaban por tres motivos: acreditaban la precisión del chart entero a cada patrón, guardaban solo el máximo y decaían la media un 10 % al día sin incertidumbre. wolluf evita las tres cosas.

### Dificultad

4. La SR oficial de mania es densidad más strain por columna y no modela manos. El rework de Sunny (PR #36342) está cerrado pero "NO cancelado".
5. MinaCalc tiene soporte 7K, pero débil: 4 pattern mods, la columna 4 asignada a la mano izquierda e ignora las LN. `minacalc-rs` (v515) acepta 4, 6 y 7K.
6. El código de Sunny no tiene licencia. Hay que reimplementarlo desde el PDF o partir de ports MIT (zzzzv C#, Metron).

### Datos locales verificados (auditoría)

7. **Biblioteca:** 18 905 charts 7K (18 589 MD5 únicos). 8755 etiquetas ordinales:
   - BMS: 7755 en 8 tablas;
   - O2Jam: 745;
   - **KomeijiDove: 120 charts base = 8 skills × 15 niveles (0th–Stellium)**, las únicas etiquetas que dan skill **y** nivel;
   - Jinjin: 14 Regular + 14 LN + 11 del LN antiguo.

   Faltan los cursos Stellium.
8. **Las etiquetas Jinjin/KomeijiDove se explican casi por completo con la densidad** (Spearman con NPS de 0.91–0.99). La información real está en lo alto de las tablas BMS (st 0.52, oj 0.32) y en O2Jam [H] (0.52). Ningún chart tiene a la vez etiqueta Jinjin y BMS, así que para unir escalas hace falta un modelo de contenido más solapamiento de jugadores.
9. **Los ejes Regular corregidos son jack, tech, speed y STREAM** (no stamina), según el set 1877727 y el registro de mania-hub. La stamina se modela aparte, a partir de los dans marathon de 450–600 s.
10. **scores.db tiene 4338 scores 7K y 4335 tienen su replay en `Data/r`.**
    - Los nombres de archivo siguen el formato `<md5>-<FILETIME>`, lo que permite enlazarlos a scores.db sin descomprimir nada.
    - 2635 scores están a nombre de "TWulfZ"; otros 1630 son offline con alias ("", W, Wulf…) y hay que confirmar que son suyos.
    - **En este cliente (20260924) los fails SÍ se guardan** (79 fails desde 2026-04), así que tosu pasa de obligatorio a opcional (solo sirve para live y para abandonos).
    - Las afirmaciones del usuario cuadran: 9th Regular 94.07 %; S en 9th Jack (96.54) y Tech (95.37); A en Speed y Stream. En LN 10th, S solo en Inverse.
    - **Su eje más débil es Speed** (10th 86.51 %) y el más fuerte Jack.
    - Juega LN en **ScoreV2**, donde cabeza y cola se juzgan por separado.
11. **El re-juzgado de replays funciona para rice:**
    - 98.3 % de paridad exacta sin mods en ScoreV1, 30/30 con Mirror, 100 % con DT/HT (ventanas `floor(base×rate)` en tiempo de mapa).
    - **LN es aproximado:** 3–16 % de paridad exacta (V1) y 4 % (V2), con un 0.2–0.45 % de judgements distintos.
    - V2 + rate está casi sin modelar (4/153).
    - Se porta el ruleset de `prelude` (MIT) con varios arreglos:
      - notelock correcto;
      - borde tardío = OK−1;
      - con V2+HR, la ventana MISS sin escalar;
      - MAX de V2 calculado con `DifficultyRange` de lazer;
      - parser `.osr` propio (osrparse descuadra el tiempo hasta varios segundos si hay skip).
    - **Hay que investigar los `.osg`** (4666 archivos en `Data/r`). Tienen cabecera 20260924 y registros por objeto, así que quizá guardan los judgements del propio stable, lo que resolvería el problema de LN.
12. **Audio:** el 94.9 % de los mapas usa mp3 y el ~5.4 % son keysounded. El usuario ya hace escaleras de rate con osu-trainer en pasos de 0.04–0.07.

### Licencias y ToS

13. `prelude/` es MIT (`interlude/` y `online/` son GPLv3). Se puede reusar `algorithms/` de mania-hub (MIT) y las tablas de LeoBlack (MIT). tosu es LGPL/GPL, así que corre como proceso aparte. Los dumps de data.ppy.sh no se pueden usar en producción sin permiso. La API prohíbe la recolección masiva; wolluf solo toca datos del propio usuario.

## Veredicto: ¿IA o algoritmo?

**Algoritmo determinista más un modelo estadístico bayesiano pequeño por jugador. Deep learning no.**
- Los patrones se detectan con reglas.
- La skill se modela como θ_a con incertidumbre por eje.
- El ML solo entra como calibrador del lado del chart (ridge/GBM monótono sobre etiquetas).
- El LLM, si se usa, solo explica.

mania-hub, el tracker más avanzado que existe, tampoco entrena nada sobre resultados. Ahí está la oportunidad.

## Arquitectura

```
wolluf/
  docs/research/ docs/adr/
  crates/
    chart/       # .osu -> Row{t_ms, tap/ln_head/ln_tail/ln_hold masks: u16}; keymode-generic
    layout/      # per-user column->finger/hand map (presets: 3|1+3 right thumb, 3+1|3 left thumb, 4|3, 3|4, both thumbs)
    patterns/    # rule engine (prelude + MinaCalc MIT rules) -> segments {axis, pattern, t0..t1, purity}; engine_version
    difficulty/  # sunny/ clean-room (J,X,P,A,R,C,Ks per window), rosu-pp (display SR), minacalc-rs (secondary),
                 # LeoBlack 7K interval tables (MIT) as overall dan prior; d_{s,a} per segment
    stable/      # osu!.db / scores.db / collection.db readers, own .osr parser, .osg decoder (TBD), re-judge engine
    player/      # θ_a ~ N(μ,σ²) per axis, Laplace/Kalman update per play with per-play offset, σ inflation (rust)
    session/     # segmentation, per-axis evidence, CUSUM/EWMA fatigue, session report
    recommend/   # (chart|section, rate) candidates, P(success), modes, "why"
    drills/      # overlay cut + rendered drill (.osu rewrite, audio), drill registry, .osz export
  app/src-tauri/ app/ui/   # Tauri 2 + React/Vite
  tools/         # CLI: index library, rejudge harness, calibration reports
```

**Crates y dependencias:**
- `rosu-map`, `rosu-pp` 4.0.1, `minacalc-rs` 515.2.0
- `osu-db` en la versión que parsee el formato 20260924 (si no, escribimos nuestro propio lector; el audit ya tiene uno en Python)
- `rusqlite`, `notify`, `tokio-tungstenite` (tosu opcional)
- `rosu-v2`
- audio: `symphonia` (MPL) → `signalsmith-stretch` (MIT, cambia el tempo sin tocar el pitch) o `rubato` (tempo y pitch juntos) → `vorbis_rs` (salida .ogg; así evitamos LAME/LGPL y el delay de mp3)

## Modelo de dominio

**Ejes (config por keymode, datos 7K en el MVP):**
- **Regular:** jack, tech, speed, stream, más stamina como eje derivado.
- **LN:** general, tech, inverse, release.
- **Patrones hoja:**
  - chordstream (ligero y denso), jumpstream y handstream;
  - brackets, trill, jumptrill y split trill, rolls y stairs;
  - chordjack, minijack y longjack, anchor;
  - patrones de pulgar (col 4);
  - LN density, LN chords, shields, release timing, inverse gaps y hybrid.

**Selección de jugadores (identidad):**
- `scores.db` mezcla varios jugadores: tus alias y replays descargados de otros.
- La app lista todos los nombres encontrados, cada uno con su número de plays y rango de fechas.
- Por defecto selecciona solo el usuario actual. El `Username` del cfg de osu! puede venir corrupto (por ejemplo `TWulfZasdasdasd d jSS||`), así que se identifica combinando: prefijo o coincidencia aproximada con el cfg, el nombre más frecuente y la cuenta de API vinculada.
- El usuario puede elegir varios nombres o todos.
- La skill se calcula solo sobre la identidad seleccionada. Las plays de otros siguen visibles para compararse, pero no contaminan el perfil.

**Unidad de evidencia:** un segmento del motor de patrones (2–8 s con una etiqueta de eje) con su dificultad d_{s,a}. **Solo las notas del segmento cuentan para su eje.**

**Modelo del jugador:**
- Residual r_s = y_s − f(μ_a − d_{s,a}).
- El offset compartido por play (τ_p) evita que 200 segmentos de un mismo play parezcan 200 pruebas independientes.
- La escala θ está **en unidades de dan Jinjin**, la misma que la dificultad de los charts.
- Entre sesiones solo crece σ (tomando como semilla el ritmo de periodos de Lichess). La media no decae.
- Los drills pesan 0.7, y 0.7^k en la repetición k-ésima.
- Se excluyen las plays con HO/NR, Random y vibro.
- Dos sistemas de precisión LN: ScoreV1 y ScoreV2.
- **Almacenamiento:** las observaciones crudas por nota se guardan de forma permanente, y la skill se puede recalcular cuando cambia `engine_version`.

**Tracker de sesión:**
- **Segmentación:** una sesión nueva tras 120 min de inactividad; bloques de 10 min; los primeros ~5 min de cada bloque cuentan como warm-up; el día empieza a las 04:00.
- **Métricas por eje:**
  - residual;
  - sesgo y dispersión por dedo o mano (mediana y 1.4826·MAD), según el layout;
  - en LN, cabeza, release y releases tempranos/tardíos;
  - clustering de misses (D > 1.5 marca una sección donde el jugador se atraganta);
  - consistencia entre intentos;
  - pendiente de stamina.
- **Reglas de decisión:**
  - mínimo ≥3 plays y ≥20 segmentos por eje (si no, se muestra "sin datos suficientes");
  - "mejoró (provisional)" con P ≥ 0.80 e intervalo de credibilidad del 80 %;
  - "confirmado" solo si se repite en la sesión siguiente;
  - "empeoró" solo con ≥2 sesiones sin fatiga (una sola sesión mala es un "mal día").
- **Fatiga:** CUSUM sobre los residuales y EWMA sobre la dispersión de timing (hipótesis a validar con los datos del usuario).
- **Reporte:** 2–3 puntos que mejoraron, 2–3 que empeoraron y qué practicar a continuación, con secciones concretas y una pista por dedo (por ejemplo, "pulgar col 4 +7 ms tarde en jacks").

**Recomendador:**
- Candidatos: pares (chart o sección, rate) con rate de 0.8 a 1.5 en pasos de 0.05.
- Se calcula P(acc ≥ A* | θ, d), con ventanas según el modo:
  - warmup: 0.85–0.95;
  - training: ~0.7;
  - push: 0.35–0.5.
- Puntuación = pureza del eje × cercanía a la P objetivo × bonus de información × relevancia para la meta (Gamma).
- Modos: Push, Consistency, Deficit y Rust.

**Drills:**
- Hay dos tipos:
  - **overlay cut** a 1.0x: solo se filtran los HitObjects y se mantienen el audio y todos los timing points; tarda menos de 1 s;
  - **rendered drill**: rate + reps + pre-roll ≥2 s + fades, salida .ogg.
- **Timing:**
  - t' = O_k + (t − (a − p))/r;
  - se re-emiten la línea roja con la fase de beat preservada y la SV activa;
  - las colas LN nunca se recortan;
  - los cortes van en downbeats sin partir acordes.
- Los keysounded solo se permiten a 1.0x.
- Registro `drill_md5 → {source_md5, a, b, p, rate, reps, O_k[]}`, que permite mapear los replays de vuelta a la sección original.
- **Escalera de rate:** pasos de ±0.05; sube con 2 de 3 reps en el objetivo y baja con 2 reps por debajo de objetivo −3 pp. Una sección solo cuenta como "dominada" tras una prueba de transferencia: el mapa completo a 1.0x en otra sesión.
- La importación se hace abriendo el `.osz` con el shell, con un manifiesto de limpieza. `collection.db` solo se escribe con osu! cerrado y con backup previo.

## Roadmap

- **F0. Base.**
  - Scaffold del workspace, Tauri 2 y React.
  - `docs/research/` (hallazgos verificados) y ADRs: IA vs algoritmo, licencias, fuentes de datos, ejes.
  - Portar a Rust los parsers de `osu!.db` y `scores.db` validados en el audit.
  - **Spike del formato `.osg`**, que puede ahorrar el trabajo de juzgar LN.
- **F1. Charts.**
  - Crates `chart`, `layout`, `patterns` y `difficulty`, más la CLI que indexa Songs a SQLite.
  - **Criterios de salida:**
    - superar a NPS en bms_st, bms_oj y O2Jam [H], y en pares del mismo dan con distinta skill;
    - predecir el slot de skill de los 120 charts KomeijiDove (8 clases, agrupando por nivel);
    - precisión por patrón medida sobre 200–300 segmentos etiquetados a mano.
- **F2. Replays.**
  - Parser `.osr`, motor de re-judge y harness de regresión sobre toda la biblioteca.
  - **Criterio de salida:** ≥98 % de paridad en rice sin mods; las métricas LN basadas en offsets (no en tiers) quedan marcadas con su nivel de confianza.
- **F3. Jugador + sesión + recomendación.**
  - Modelo θ_a, tracker de sesión, reporte y recomendador.
  - **Criterios de salida:**
    - con split temporal (entrenar antes del 2026-09-01 y probar en septiembre, 897 plays), el log-loss y el Brier superan a las bases de SR y NPS;
    - el modelo reproduce Jack como el eje más fuerte y Speed como el más débil.
- **F4. Drills.** Overlay cut y rendered drill, registro, escalera de rate, `.osz` e integración con la sesión.
- **F5. Después del MVP.**
  - 4K (MinaCalc 4K es fuerte);
  - calibrador ML;
  - datos cross-player opt-in (o acuerdo con mania-hub/ppy);
  - LLM para explicaciones;
  - Linux+Wine.

## Entorno (2026-09-28)

- osu! stable está en `/mnt/e/Games/osu!` (el usuario autorizó acceso de solo lectura).
- En WSL están Rust 1.98.1 (en `~/.cargo/bin`), webkit2gtk-4.1, node 24, pnpm y git. El repo tiene `git init` en la rama `main`.
- WSL no puede leer la memoria de osu!. Como tosu ya es opcional, F0–F3 se pueden desarrollar íntegros en WSL sobre archivos locales; la E2E en vivo queda para una build de Windows.

## Riesgos y decisiones abiertas

- El juicio de LN no es exacto: depende del spike de `.osg` o de aceptar métricas basadas en offsets.
- La escala LN 7K cuenta con pocas etiquetas (60 KomeijiDove LN, 14 dans y O2Jam de peso bajo).
- Unir escalas (Jinjin ↔ BMS) exige un modelo de contenido.
- Pendiente: confirmar los alias offline del usuario.
- La técnica (manip/vibro) no se ve en la precisión, y la UI debe decirlo.

## Verificación

- **Unitarias:** parser, reglas de patrones, reescritura de timing en drills (round-trip y fase de beat) y ventanas de judgement.
- **Harness de re-judge** sobre las 4356 replays 7K, con paridad por grupo de mods × proporción de LN.
- **Calibración** contra etiquetas (Spearman/Kendall frente a NPS).
- **Modelo:** holdout temporal (log-loss y Brier).
- **E2E en Windows:** jugar → replay → θ se mueve → reporte de sesión → drill importado en stable.
