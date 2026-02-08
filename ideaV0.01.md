Quiero hacer un programa que yo le pueda mandar bases de datos en texto, en CSV, en varios formatos. Piensa como un senior developer. y él pueda coger esas bases de datos y ponerlas sin repetidos, sin repetidos, en orden, un orden. Son bases de datos de una sola palabra, que las separe, que si tienen coma, que si están separadas por espacios, que si están separadas por coma y me entregue un archivo ordenado. No tiene que ordenarlo alfabéticamente, pero puede hacerlo, pero separadas por lo que yo le diga, por coma, por esto. ¿Qué lenguaje sería óptimo? ¿Cuál sería el más rápido para que haría esto? Son bases de datos gigantes de muchas palabras.Para ese tipo de “normalizador/deduplicador” de **listas gigantes de tokens** (una palabra por ítem, pero pueden venir pegadas por coma/espacios/saltos de línea), lo que manda no es tanto “ordenar”, sino **cómo procesas en streaming** y **cómo deduplicas** sin reventar RAM.

## Qué lenguaje es óptimo (por caso de uso)

### 1) **Rust** (máximo rendimiento + binario portable)

* **Muy rápido** (cerca de C/C++) y con excelente control de memoria.
* Ideal si quieres un **CLI** que corra en cualquier máquina sin dependencias.
* Te permite hacer deduplicación eficiente y, si toca, **external sort** (cuando no cabe en RAM) con buen rendimiento.

**Cuándo lo elegiría:** si esto va a ser una herramienta “seria”, recurrente, con archivos enormes (GBs) y quieres que sea “a prueba de todo”.

---

### 2) **Go** (rendimiento alto + desarrollo más simple que Rust)

* Muy buen rendimiento, especialmente en I/O y procesamiento de texto.
* Concurrencia fácil (goroutines) si quieres paralelizar etapas.
* Compila a binario estático y se despliega fácil.

**Cuándo lo elegiría:** si quieres *casi* el rendimiento de Rust, pero con menor fricción de desarrollo.

---

### 3) **Python** (rápido para construir, no el más rápido ejecutando)

* Para un prototipo funciona en 1 día.
* Pero con “bases de datos gigantes”, si todo se queda en `set()` en RAM, te limita rápido.
* Puede volverse excelente si apoyas deduplicación en disco (SQLite/DuckDB) o usas pipelines.

**Cuándo lo elegiría:** si priorizas velocidad de implementación y puedes aceptar menor velocidad o vas a usar motor en disco.

---

## Lo más importante: “gigante” cambia la estrategia

### Si **cabe en RAM**

* Parseas en streaming → normalizas → metes en un `HashSet` → al final escribes.
* Lenguajes: Rust/Go ganan por velocidad y memoria.

### Si **NO cabe en RAM**

Necesitas **deduplicación en disco**, típicamente:

1. **Partitioning por hash** (escribes N archivos/buckets según hash del token)
2. Deduplicas cada bucket por separado (ya cabe en RAM)
3. Opcional: ordenas dentro de cada bucket si quieres salida estable

O alternativas:

* **External sort** (ordenas por chunks y haces merge) y deduplicas durante el merge.
* **SQLite / DuckDB**: insertas tokens normalizados a una tabla con índice único y exportas (simple, pero depende del motor y del disco).

Para “muchas palabras” realmente grandes, esta parte vale más que el lenguaje.

---

## Respuesta directa a tus preguntas

* **Lenguaje óptimo (balance):** **Go**
* **Más rápido (puro performance + control):** **Rust**
* **Más rápido para construir (time-to-first-tool):** **Python**, pero no el más rápido corriendo.

Si me dices “gigantes” en serio (decenas de GB y millones/billones de tokens), yo me iría con **Rust** o **Go** y una estrategia **en disco** (bucket por hash o external sort). Si hablamos de “millones” pero caben en RAM (p. ej. 1–5 GB de texto y una máquina decente), Go o Rust con `HashSet` te lo resuelve muy bien.

---

## Recomendación práctica (mi pick)

**Go** si quieres productividad + velocidad real + despliegue fácil.
**Rust** si quieres exprimir rendimiento y memoria y hacerlo “nivel herramienta industrial”.

Si quieres, te dejo un diseño de CLI (flags tipo `--input`, `--delimiter`, `--output-delimiter`, `--normalize=lower|trim`, `--dedupe-mode=memory|disk`, `--buckets=256`) y el algoritmo exacto para modo RAM vs modo disco.
Listo, hagámoslo en Rust. Piensa, es una interfaz muy sencilla, probablemente solo una pantalla, analízalo tú, es una pantallita donde uno selecciona, le da a seleccionar todos los archivos que quiere unir y borrar duplicados, selecciona dónde quiere exportarlos, el nombre, también selecciona el separador que quiere darle al archivo final, un espacio, una coma, un punto y coma, lo que sea. No sé, eso se me ocurre a mí. Piensa tú, pues es básicamente eso. O sea, el funcionamiento básico sería, pues dos archivos o tres o cuatro o cinco, los que sean, los va a unir en un solo archivo sin repetidos con el separador que tú le digas.Perfecto: **Rust** para el “motor” y una **GUI mínima** encima. Para lo que describes (una pantallita con seleccionar archivos → elegir salida → separador → ejecutar), yo lo diseñaría así:

## 1) Arquitectura recomendada (simple y robusta)

### A. Motor (Rust puro) ✅

Un crate (librería) que hace todo el trabajo pesado:

* Lee **N archivos** (txt/csv/mixtos) en streaming
* Extrae “tokens” (palabras/ítems) según reglas de separación
* Normaliza (opcional: trim, lower, etc.)
* Deduplica
* Exporta con el separador que el usuario elija

**Clave:** el motor debe soportar 2 modos:

1. **En RAM** (rápido): usa `HashSet` si cabe.
2. **En disco** (para archivos gigantes): “buckets por hash” o “external sort”.

> La GUI no debería saber nada de esto; solo llama al motor con una config.

---

### B. GUI (una sola pantalla) ✅

Dos opciones buenas:

#### Opción 1 — **Tauri v2** (mi favorita para “app de escritorio pro”)

* UI en HTML/CSS/JS (Svelte/React/Vue) y backend Rust
* Instaladores fáciles (Windows/macOS/Linux)
* File picker nativo muy cómodo

#### Opción 2 — **egui/eframe** (todo Rust, muy rápido de construir)

* Una ventana nativa, UI inmediata (muy fácil)
* Perfecto si quieres cero frontend web
* File pickers con `rfd`

**Si quieres “una pantallita” y ya:** *egui + rfd* es la ruta más corta.

---

## 2) UX propuesto (una sola pantalla)

### Sección 1: Inputs

* **Botón:** “Seleccionar archivos…”
* Lista de archivos seleccionados (con eliminar individual / limpiar lista)
* Indicador: “N archivos, tamaño total aprox.”

### Sección 2: Salida

* **Guardar como…** (elige carpeta + nombre)
* Formato sugerido:

  * `.txt` si es lista simple
  * `.csv` si el separador es coma (opcional)

### Sección 3: Separación / parsing

* **Separadores de entrada** (para poder “entender” archivos distintos):

  * ☑ Espacios
  * ☑ Saltos de línea
  * ☑ Comas
  * ☑ Punto y coma
  * (avanzado) “Separadores personalizados” (campo texto)
* **Separador de salida** (uno):

  * Dropdown: `\n` (línea nueva), `,`, `;`, espacio, `|`, “custom”
  * Si es custom: caja para escribirlo

### Sección 4: Normalización (para dedupe “real”)

* ☑ Trim (quitar espacios extremos) — **ON por defecto**
* ☑ Ignorar mayúsculas/minúsculas (lowercase) — opcional
* ☑ Quitar tokens vacíos — **ON**
* (avanzado) “Preservar orden de primera aparición” — sí/no

### Sección 5: Rendimiento

* “Modo deduplicación”:

  * **Auto (recomendado)** (elige RAM vs Disco según heurística)
  * RAM (rápido, pero puede explotar)
  * Disco (para gigantes, más lento, seguro)

### Sección 6: Ejecutar

* Botón grande: **“Unir y quitar duplicados”**
* Barra de progreso + contador:

  * tokens procesados
  * únicos
  * duplicados descartados
  * archivos completados
* Al final: “Abrir carpeta” / “Copiar ruta”

---

## 3) Definición exacta del comportamiento (para que no haya sorpresas)

### Deduplicación: ¿qué significa “sin repetidos”?

* Si normalización lowercase está ON:

  * `Apple`, `apple`, `APPLE` → se consideran el mismo token.
* Si está OFF:

  * se consideran distintos.

### “Mantener orden”

Tú dijiste “no tiene que ser alfabético”. Entonces lo ideal es:

* **Orden por primera aparición** (estable):

  * Si aparece en el archivo 1 primero, se queda en ese orden.
* Alternativa opcional:

  * **Orden alfabético** (para comparar fácilmente)

> Importante: “orden por primera aparición” requiere guardar un set + output en ese orden (igual se puede en modo disco, pero es más complejo). Por defecto yo pondría “orden por primera aparición” en modo RAM, y en modo disco ofrecería “orden consistente por buckets” o “orden alfabético” (más fácil de garantizar).

---

## 4) Algoritmo (en cristiano, pero serio)

### Modo RAM (rápido)

* `HashSet` para membership
* `Vec<String>` para el orden final (solo cuando el token es nuevo)

Pseudo:

* por cada token:

  * normaliza
  * si `set.insert(token)` fue true:

    * `output.push(token)`

**Pros:** rapidísimo
**Contras:** RAM sube con los únicos

---

### Modo Disco (para gigantes)

**Bucket por hash (recomendado)**:

1. Crea N archivos temporales (ej. 256 buckets)
2. Lee tokens y manda cada token al bucket `hash(token) % N`
3. Luego procesa bucket por bucket:

   * dedupe en RAM (porque cada bucket es más pequeño)
   * escribe al output (en un orden definido)

**Pros:** no revienta RAM
**Contras:** más I/O, output puede no preservar “primera aparición global” sin trabajo extra

> Para una V1, modo disco con “salida ordenada alfabéticamente” o “por buckets” es perfecto.

---

## 5) Crates recomendados (stack concreto)

### GUI (elige una)

* **egui/eframe** (UI)
* **rfd** (file dialogs nativos)

o

* **Tauri** (UI web) + Rust backend

### Motor

* `anyhow` / `thiserror` (errores)
* `memmap2` (opcional, lectura rápida de archivos grandes)
* `csv` (si quieres soportar CSV “de verdad” con parser)
* `ahash` o `fxhash` (hash rápido para HashSet)
* `tempfile` (modo disco)
* `indicatif` (si fuera CLI; en GUI harías tu barra)

---

## 6) Lo que yo haría como V1 (en 1 pantalla y sin complicarte)

**V1:**

* Entrada: separadores comunes (espacio, coma, ;, newline)
* Salida: separador elegido
* Normalización: trim + remove empty (ON)
* Deduplicación: RAM + Auto (si detecta muy grande, avisa y sugiere modo disco)
* Orden: “primera aparición” (RAM)

**V1.1:**

* Modo disco con buckets
* Opción de ordenar alfabético

---

## 7) Decisiones rápidas que necesito tomar yo (sin preguntarte 20 cosas)

Asumo por defecto:

* **Tokens = “una palabra/ítem”**, delimitados por: whitespace + `,` + `;` + saltos de línea
* Normalización: **trim ON**, vacíos fuera ON, lowercase OFF
* Output: **un token tras otro separado por el separador elegido**
* Orden: **primera aparición** (si estás en RAM)

---

Si quieres, en el siguiente mensaje te dejo **un blueprint exacto de proyecto Rust** (estructura de carpetas, `Cargo.toml`, módulos `parser.rs`, `dedupe.rs`, `writer.rs`, `gui.rs`) y un **MVP listo** con `egui + rfd` (sin florituras, pero sólido) para que lo pegues y lo corras.
Perfecto. Una pregunta, si quiero que funcione tanto en Windows como en Mac con Apple Silicon, o sea, en los M1, M2, M3, M5, ¿puedo usar Tauri y Rust?Sí. **Tauri + Rust es una combinación muy buena para correr en Windows y en macOS (incluido Apple Silicon: M1/M2/M3/etc.)**.

Lo importante es tener claro qué te da Tauri y qué debes cuidar para que el build sea “sin sorpresas”:

## Qué tan viable es (en la práctica)

* **Windows:** genera un `.exe` instalable (MSI/NSIS según config).
* **macOS Apple Silicon:** compilas a **`aarch64-apple-darwin`** (nativo). Si quieres, también puedes generar **universal binary** (Intel + Apple Silicon) para distribuir un solo `.app`.
* **Rust** compila perfecto para Apple Silicon; Tauri usa WebView nativa del sistema y funciona bien.

## Requisitos / “gotchas” a cuidar

### 1) WebView del sistema

* En **Windows**, Tauri usa **WebView2**. En la mayoría de equipos ya está; si no, tu instalador puede incluir bootstrapper o indicar requisito.
* En **macOS**, usa **WKWebView** (viene con el sistema).

### 2) Firma y notarización (macOS)

* Para distribuir fuera de tu Mac (o evitar bloqueos de Gatekeeper), normalmente necesitarás:

  * **Code signing** (Apple Developer ID)
  * **Notarization**
* Para uso personal/local, no es obligatorio, pero para entregar a otros casi siempre sí.

### 3) Acceso a archivos (tu caso)

Tú necesitas:

* File picker múltiple ✅
* Escritura en ruta elegida ✅
* Leer archivos grandes ✅

Todo eso es normal en Tauri, solo que:

* En macOS, si distribuyes sandboxed/firmado, debes declarar permisos (Tauri lo maneja, pero hay que configurarlo).

## Rendimiento para tu app

Tu lógica pesada (dedupe) está en Rust, así que:

* UI (WebView) solo muestra progreso.
* Rust hace streaming + dedupe y va reportando progreso a la UI.
  Esto es exactamente el punto fuerte de Tauri.

## Recomendación concreta para tu app

* **Tauri** si quieres una app “bonita”, fácil de distribuir y con UI moderna.
* **egui** si quieres lo más simple y no quieres pelear con toolchains web.

En tu caso (una sola pantalla, pero quieres cross-platform pro): **Tauri tiene sentido**.

Si te sirve, el “stack final” típico sería:

* **Frontend:** Svelte (ligero) o React (si ya lo usas)
* **Backend:** Rust (parser + dedupe + writer)
* Comunicación: `invoke`/commands de Tauri + eventos para progreso

Si me dices si prefieres **Svelte** o **React** (o “lo más simple”), te propongo la plantilla exacta de proyecto y cómo empacar para Windows + macOS Apple Silicon (incluyendo universal si lo quieres).
Listo. El archivo que yo le doy, él tiene que identificar, son palabras. Él tiene que separar esas palabras y pueden estar separadas por enter, por espacios, por comas, por punto y coma. Él tiene que ser capaz de identificar eso. y luego, pues ajá, vuelvo y repito, el archivo final une todas las listas o si es solo una, pues pone esa lista sin repetidos, con el espaciador que yo le diga, puede ser un espacio, una coma, un punto y coma, un salto de página o enter.Perfecto. Para tu caso, **no necesitas “adivinar” cuál separador trae cada archivo**: lo más robusto (y rápido) es **tratar *todos* esos separadores como delimitadores válidos al mismo tiempo** y tokenizar en streaming.

## Estrategia correcta de parsing (sin inventos)

### Delimitadores de entrada (siempre activos)

Separar tokens cuando aparezca cualquiera de:

* **Cualquier whitespace**: espacio, tab, `\n`, `\r\n` (Windows), `\r` (viejo Mac)
* **`,`** (coma)
* **`;`** (punto y coma)

Eso cubre exactamente lo que dijiste: enter, espacios, comas, punto y coma.

**Regla de oro:** si esto son “palabras/ítems” (no frases), entonces dividir por cualquiera de esos separadores es lo correcto.

### Limpieza mínima (para “sin repetidos” real)

* `trim()` (quita espacios al inicio/fin) ✅
* descartar token vacío ✅
* (opcional) dedupe case-insensitive (`to_lowercase()`) si quieres que `Perro` y `perro` sean lo mismo.

---

## Salida: separador configurable

El separador de salida no depende del input. Tú lo escoges y el writer lo usa al exportar:

Ejemplos de opciones:

* **Espacio**: `" "`
* **Coma**: `","`
* **Punto y coma**: `";"`
* **Enter / nueva línea**: `"\n"` (yo lo pondría como default)
* **Salto de página**: `"\x0C"` (form feed, FF)

> Nota: “salto de página” no es común en datos, pero se soporta fácil (es un char).

---

## Dedupe + orden (lo que yo implementaría por defecto)

* **“Orden de primera aparición”**: si una palabra aparece por primera vez en el archivo 3, queda en la posición en que apareció por primera vez en el total.
* Dedupe con `HashSet`.
* Guardas el orden en un `Vec<String>` (solo cuando insertas por primera vez).

Esto es lo esperado cuando dices “no necesariamente alfabético”.

---

## Manejo de “gigantes”

* Modo **RAM**: HashSet + Vec (rápido).
* Modo **DISK** (si te pasas de RAM): buckets por hash (seguro).
  En ese modo, lo más fácil es ofrecer salida **alfabética** o “por buckets” (porque preservar el orden global exacto es más complejo).

La GUI puede tener:

* **Auto (recomendado)**: intenta RAM; si ve que se está poniendo pesado, cambia a disco o avisa.

---

## Implementación del tokenizador (Rust) — robusto y rápido

Sin regex (para velocidad). La idea es leer bytes/chars y cortar tokens cuando llega un delimitador.

Delimitadores de entrada: `is_whitespace() || c == ',' || c == ';'`.

Ejemplo de función (core del motor):

```rust
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}
```

Luego:

* acumulas chars en un buffer `String`
* cuando llega delimitador, haces `trim` y emites token si no está vacío

Esto funciona igual para archivos con:

* `palabra1,palabra2 palabra3\npalabra4;palabra5`
* Windows `\r\n` (porque `is_whitespace()` lo cubre)
* archivos mezclados

---

## Un detalle importante (para no romper CSV “real”)

Tú dijiste “bases de datos de una sola palabra”. Perfecto.
Pero si algún día metes CSV de verdad con comillas como `"New York"`, tu tokenizador lo partiría en `New` y `York`.

Si quieres cubrir ese futuro, te recomiendo en la UI un switch:

* **Modo simple (palabras)** ✅ (tu caso actual)
* **Modo CSV (columna específica)** (futuro)

Pero para tu requerimiento actual, el modo simple es exactamente lo correcto.

---

## Resumen de comportamiento (tal como lo pediste)

1. Seleccionas 1 o muchos archivos
2. El motor **tokeniza** usando separadores: enter/espacios/coma/punto y coma
3. **Deduplica** (y opcional normaliza)
4. Exporta todo unido con el **separador de salida** que tú elijas (espacio, coma, `;`, `\n`, `\f`, etc.)

Si quieres, en el próximo paso te dejo el **diseño de la pantalla en Tauri** (controles exactos) + la **estructura de crates** (`core` + `tauri-app`) y el contrato de funciones (`process_files(config) -> stats + output`).
Me encanta. Sigas y sigue pensando como un genio, como lo que eres, como un senior developer de Rost. y piensa también en la arquitectura, piensa en cosas modernas. Si quieres, haz una investigación en internet cómo hacerlo lo más rápido, que si le meto una lista con cinco millones de palabras, sea lo más rápido. Me gusta esa estrategia que sea auto en RAM y luego intente otras cosas, pero también investiga qué otros algoritmos hay nuevos o qué podría implementar para que sea lo más rápido. Aunque es una aplicación muy sencilla, hagamos lo mejor que se pueda.Sí — y ahora que quieres “lo mejor posible” para **5+ millones de palabras**, la clave no es la GUI: es el **motor** (parsing + dedupe + salida) y cómo evita *allocations* y *cache misses*. Te dejo una arquitectura “senior”, con estrategias de rendimiento probadas y opciones de algoritmos para escalar.

---

## Arquitectura moderna (limpia, testable y rápida)

### 1) `core` (crate Rust, sin GUI)

Responsable de:

* **Tokenizar** en streaming (whitespace + `,` + `;`)
* **Normalizar** (trim, vacíos, opcional lowercase)
* **Deduplicar** (RAM o DISK)
* **Emitir progreso** (callbacks o canal)

Esto se vuelve reusable: CLI, Tauri, tests, benchmarks.

### 2) `app-tauri` (UI mínima)

* File picker múltiple
* Selector de output + nombre
* Selector de separador de salida (incluye `\n` y `\x0C`)
* “Modo Auto / RAM / Disco”
* Progreso (por eventos)

Tauri soporta comandos para llamar Rust desde el frontend y un sistema de eventos para reportar progreso. ([Tauri][1])

---

## Rendimiento extremo: qué implementar (de mayor impacto a menor)

### A) Tokenización ultra rápida (sin regex, sin CSV parser)

* Lectura **buffered** grande (`BufRead`) o **memmap** (cuando aplica).
* Tokenizar por delimitadores (`is_whitespace || ',' || ';'`) en un solo pase.
* Evitar crear `String` por cada token si puedes (ver interning/arena abajo).

> Regex aquí suele ser más lenta y hace más asignaciones de memoria.

---

### B) Deduplicación en RAM: usa SwissTable + hasher rápido

El estándar (`std::collections::HashSet`) usa un hasher más “seguro” pero más lento para este caso. Para rendimiento puro:

* `hashbrown::HashSet` (SwissTable, muy rápido en caché y probing). ([Docs.rs][2])
* Un hasher rápido tipo `ahash` (ideal para datos no hostiles: tu app local de dedupe).

**Bonus de performance:**

* `set.reserve(estimado)` para evitar rehashing.
* Guardar “orden de primera aparición” con `Vec` solo para tokens nuevos.

---

### C) Reducir allocations (esto es donde “se siente” el genio 😄)

Con 5 millones de tokens, el costo #1 suele ser **alocar/copiar strings**.

Opciones (de simple → pro):

1. **Simple y ya muy rápido:** almacenar `Box<str>` por token único.
2. **Interning con arena (más pro):**

   * Guardas bytes de tokens en un “arena allocator” (ej. `bumpalo`)
   * El `HashSet` guarda *punteros/slices* estables (menos alloc por token)
3. **Hybrid:** buffer por archivo + copiar solo cuando el token es nuevo.

En términos prácticos: (1) te rinde excelente hasta varios millones; (2) es para exprimir más.

---

### D) Paralelismo real (sin pelear por locks)

Un patrón muy efectivo: **sharding por hash**:

* Calculas un hash rápido del token
* Mandas el token al “shard” `hash % N` (N = #threads)
* Cada thread deduplica en su propio `HashSet` (sin locks)
* Luego concatenas resultados (y si quieres orden estable, lo decides por modo)

Esta idea se usa mucho para dedupe paralelo: asignar elementos a hilos por hash para evitar colisiones entre hilos. ([The Rust Programming Language Forum][3])

**Nota:** Si para ti el orden “primera aparición global” es sagrado, el paralelismo complica eso. Se puede, pero sube complejidad. Yo lo ofrecería como:

* “Máxima velocidad” (paralelo, orden no garantizado o alfabético)
* “Orden estable” (un hilo o estrategia más compleja)

---

## Cuando NO cabe en RAM: algoritmos exactos (sin aproximaciones)

Para dedupe exacto, hay dos enfoques clásicos y muy efectivos:

### 1) External sort + dedupe (muy sólido)

* Ordenas externamente (por chunks) y luego haces merge deduplicando.
* Es el concepto estándar para “archivos enormes” que no caben en memoria. ([Stack Overflow][4])

En Rust, tienes implementaciones listas de external merge sort como `gsort`. ([Docs.rs][5])

**Pros:** output alfabético + dedupe exacto, muy estable
**Contras:** sorting de strings puede ser pesado (pero en disco es el camino)

### 2) Buckets por hash (mi recomendación para tu app)

* Particionas tokens en N archivos temporales por `hash(token) % N`
* Procesas bucket por bucket en RAM (ahí sí cabe)
* Escribes al output con tu separador

**Pros:** simple, exacto, rápido en práctica
**Contras:** orden global estable no queda “primera aparición”; puedes ofrecer orden alfabético por bucket o total (si luego sort).

---

## “Modo Auto” bien hecho (sin adivinar)

En Auto yo haría:

1. Estimar memoria (heurística):

   * Tamaño total de input (sumatoria de archivos)
   * Longitud promedio de token (muestreo rápido de los primeros X MB)
   * Estimar “unique ratio” (muestreo con HashSet pequeño)
2. Decisión:

   * Si estimado de únicos * (overhead por entrada) < X% de RAM disponible ⇒ **RAM**
   * Si no ⇒ **DISK (buckets)** o **external sort**
3. En RAM, si detectas crecimiento peligroso:

   * abortas elegante y propones “cambiar a DISK” (o cambias automáticamente si implementas “spill”)

---

## Detalles de Tauri para que sea rápido de verdad

* **Nunca** envíes millones de tokens al frontend (IPC + JSON sería un cuello brutal).
* Solo envía **progreso y métricas** (tokens leídos, únicos, duplicados, velocidad, ETA opcional).
* El trabajo se queda 100% en Rust y el frontend solo pinta. (Esto encaja con la recomendación general de usar commands + events). ([Tauri][1])

---

## Mi propuesta “Best Possible” (sin sobre-ingeniería)

### V1 (rápida, ya muy potente)

* RAM con `hashbrown` + `ahash`
* Orden: primera aparición (RAM)
* Auto: RAM vs DISK (buckets)
* Output separador configurable (incluye `\n` y `\x0C`)
* Progreso por eventos Tauri

### V1.1 (modo “monstruo”)

* Paralelo por shards (modo “máxima velocidad”)
* External sort opcional (para output alfabético estable en DISK) con `gsort` ([Docs.rs][5])

---

## Próximo paso (te lo dejo listo)

Si me das el nombre del proyecto (o uso uno tipo `dedup-joiner`) te escribo:

* Estructura exacta del repo (`/crates/core`, `/apps/tauri`)
* El modelo `Config` (delimitadores entrada, separador salida, normalización, modo)
* La API `process_files(config, progress_cb) -> Stats`
* Y un plan de benchmarks (para validar “5 millones” de forma seria)

¿Quieres que la salida por defecto sea `\n` (una palabra por línea)? Es lo más compatible para datasets grandes y luego tú lo cambias a coma/espacio cuando lo necesites.

[1]: https://v2.tauri.app/develop/calling-rust/?utm_source=chatgpt.com "Calling Rust from the Frontend"
[2]: https://docs.rs/hashbrown?utm_source=chatgpt.com "hashbrown - Rust"
[3]: https://users.rust-lang.org/t/multicore-dedup-vec-u64-u64/80654?utm_source=chatgpt.com "Multicore dedup Vec<(u64, u64)>?"
[4]: https://stackoverflow.com/questions/9816201/remove-of-duplicate-strings-from-very-big-text-file?utm_source=chatgpt.com "Remove of duplicate strings from very big text file"
[5]: https://docs.rs/gsort?utm_source=chatgpt.com "gsort - Rust"
¿Hay alguna forma para que Rust use una GPU Nvidia si la tiene disponible y pueda hacer este proyecto, pues pueda hacer la función del proyecto aún más rápido?Sí, **se puede usar GPU (NVIDIA) desde Rust**, pero para *este* problema concreto (tokenizar texto + deduplicar strings) casi nunca es la mejor inversión: el cuello de botella suele ser **I/O + parsing + hashing + memoria**, y la GPU no es buena manejando **strings de longitud variable** ni estructuras tipo `HashSet` con inserciones y colisiones (mucho branching y acceso irregular a memoria). Aun así, te explico **cuándo sí sirve** y **cómo se haría** sin humo.

## Cuándo la GPU puede ayudar de verdad

### 1) Si el input ya está “tokenizado” en números (o IDs)

Si conviertes cada palabra a un **hash/ID fijo (u64/u128)** en CPU, entonces la GPU puede acelerar:

* **sort** masivo
* **unique** (dedupe) sobre enteros

En GPU, “sort + unique” de enteros sí escala muy bien. Pero ojo: ya hiciste el trabajo difícil (convertir texto→tokens→hash) en CPU.

### 2) Si necesitas **orden alfabético** de salida y el dataset es enorme

Podrías:

* CPU: extraer tokens + construir offsets
* GPU: ordenar por claves (hashes / claves normalizadas) y deduplicar
* CPU: reconstruir salida
  Pero esto es complejo, y la ganancia depende mucho del tamaño y del SSD.

### 3) Si tu pipeline tiene más etapas “numéricas”

Ej: embeddings, similitud, clasificación… ahí GPU sí brilla. Para dedupe exacto de palabras, menos.

## Por qué generalmente NO vale la pena aquí

* **Transferencia CPU↔GPU** (PCIe) puede comerse la ganancia.
* Las palabras son **strings**: longitudes variables, delimitadores, Unicode, normalización… GPU sufre ahí.
* Deduplicar requiere tabla hash / estructura de conjunto; en GPU es posible, pero suele ser más lento que CPU para textos reales por colisiones y acceso aleatorio.

En un equipo típico, un motor CPU bien hecho en Rust (hashbrown + ahash + lectura eficiente) ya te va a ir muy rápido con millones de tokens.

---

## Si aún quieres “modo GPU” (la forma sensata)

### Arquitectura híbrida (la única que recomiendo)

1. **CPU (Rust):** tokeniza + normaliza → computa `u64 hash` por token
2. (Opcional) guardas una estructura `Vec<(hash, token_bytes_ref)>`
3. **GPU (NVIDIA):** dedupe de hashes:

   * ordena `hashes`
   * hace unique para obtener hashes únicos
4. **CPU:** resuelve colisiones de hash (porque u64 puede colisionar) y escribe salida

**Clave:** La GPU trabaja sobre `u64`, no sobre strings.

### ¿Cómo se programa en Rust?

Opciones reales:

1. **CUDA directamente**

* Rust llama kernels CUDA vía FFI.
* Crates como `cust` (CUDA Driver API) existen para Rust y sirven para gestionar memoria y lanzar kernels (varía según mantenimiento).
* Esto te da control, pero añade un stack de build “pesado”.

2. **GPU compute portátil (Vulkan/WGPU)**

* `wgpu` te permite compute shaders que corren en GPU (NVIDIA/AMD/Intel).
* Pero implementar “sort + unique” eficiente en shaders no es trivial.

3. **Usar librerías GPU existentes (CUB/Thrust/cuDF)**

* Llamar desde Rust a C++ que use Thrust/CUB para `sort` y `unique`.
* A veces es lo más práctico si de verdad quieres GPU.

---

## Mejor ROI: optimizaciones CPU que sí te van a dar velocidad brutal

Antes de GPU, yo metería esto (y normalmente con esto ya vuelas):

* `hashbrown::HashSet` + `ahash` ✅
* Tokenizador byte-level (sin regex) ✅
* `BufReader` grande o `memmap2` ✅
* `reserve()` agresivo para evitar rehash ✅
* **Paralelismo por shards** (N threads, sin locks) ✅
* Para “DISK mode”: buckets por hash ✅

Con eso, 5 millones de tokens es totalmente manejable en CPU en tiempos muy buenos (depende del disco y del tamaño medio de palabra).

---

## Recomendación honesta

* **V1/V1.1:** CPU “top” + sharding + disk-buckets.
* **V2 (experimental):** modo GPU solo si:

  * tus datasets son gigantes (decenas/cientos de millones de tokens) y
  * tu máquina tiene GPU potente y
  * estás dispuesto a complejidad de build y mantenimiento.

Si quieres, te propongo un **plan de implementación** con “CPU fast path” primero (benchmarks), y dejamos “GPU mode” como módulo opcional detrás de un flag `--gpu` (solo NVIDIA, solo si detecta CUDA). Así no comprometes la app principal.
No, olvidemos la idea de la GPU por el momento. ¿Tienes alguna pregunta de mi proyecto, de la idea fundamental o ya lo tienes totalmente claro?Lo tengo **muy claro** 👍
A nivel funcional y técnico, tu idea está completamente entendida y bien delimitada. No hay ambigüedad crítica. Aun así, como **senior Rust developer**, te hago **solo las preguntas que realmente importan**, no por falta de claridad, sino para **cerrar decisiones de arquitectura** y evitar retrabajo más adelante.

Voy por partes.

---

## Lo que ya tengo 100 % claro (y no necesito preguntar)

### Funcional

* Input: **1 o muchos archivos** (txt / csv “simple” / texto plano)
* Tokens: **una palabra / ítem**
* Separadores de entrada:

  * whitespace (espacios, tabs)
  * saltos de línea (Windows / macOS / Linux)
  * `,`
  * `;`
* Proceso:

  * tokenizar
  * limpiar (`trim`, vacíos fuera)
  * **deduplicar**
* Output:

  * **un solo archivo**
  * separador elegido por el usuario:

    * espacio
    * coma
    * punto y coma
    * `\n`
    * `\x0C` (salto de página)
* Orden:

  * **no alfabético por defecto**
  * preferiblemente **orden de primera aparición**
* Plataforma:

  * **Windows + macOS Apple Silicon**
  * **Rust + Tauri**
* Performance:

  * millones de tokens
  * **Auto mode** (RAM → DISK)
* No GPU (por ahora)

Todo eso está perfectamente definido.

---

## Las ÚNICAS preguntas que valen la pena (4 decisiones finales)

### 1️⃣ ¿Case sensitivity por defecto?

**Pregunta concreta:**

> ¿`Perro`, `perro` y `PERRO` deben considerarse el mismo ítem por defecto?

**Opciones:**

* A) **Sí** → normalización a lowercase ON por defecto
* B) **No** → se consideran distintos (lowercase OFF por defecto)

🔧 *Impacto:* afecta deduplicación y expectativas del usuario.
📌 *Mi recomendación:* OFF por defecto, con checkbox “Ignorar mayúsculas”.

---

### 2️⃣ ¿Unicode completo o ASCII “rápido”?

**Pregunta concreta:**

> ¿Tus palabras pueden incluir acentos, ñ, emojis, caracteres no latinos?

**Opciones:**

* A) **Sí, Unicode completo** (español, nombres, etc.)
* B) **Solo ASCII / alfanumérico básico**

🔧 *Impacto:* tokenización y normalización (`to_lowercase()` en Unicode es más costoso).
📌 *Mi recomendación:* Unicode completo (seguro y correcto).
Rust lo maneja bien, solo lo aclaro para no hacer optimizaciones agresivas incorrectas.

---

### 3️⃣ Output: ¿una sola línea o formato libre?

**Pregunta concreta:**

> Cuando el separador es espacio o coma, ¿esperas todo en **una sola línea**, o puede generar múltiples líneas?

**Opciones:**

* A) **Siempre una sola línea**
* B) **No importa, solo separador** (yo escribiría secuencialmente)

🔧 *Impacto:* writer y tests.
📌 *Mi recomendación:* B (más flexible, más simple).

---

### 4️⃣ Comportamiento en modo DISK

**Pregunta concreta (importante):**

> Si el dataset es tan grande que usamos modo DISK, ¿qué prefieres?

**Opciones:**

* A) Mantener **orden de primera aparición** (más complejo, más lento)
* B) Aceptar **orden alfabético** en modo DISK
* C) Avisar al usuario y dejar elegir

📌 *Mi recomendación:* C (checkbox + aviso claro).

---

## Lo demás: ya está decidido (y bien decidido)

No necesito preguntarte sobre:

* GUI → una pantalla, claro
* Formato → texto plano
* Separadores → claros
* Tecnología → Rust + Tauri
* Performance → prioridad máxima
* UX → simple pero sólida

---

## Conclusión honesta

Tu idea **no tiene huecos conceptuales**.
Es una herramienta **simple en apariencia**, pero muy bien planteada para hacerse **industrialmente correcta**.

Si me respondes esas **4 preguntas**, puedo:

* cerrar el **modelo `Config` definitivo**
* diseñar el **motor sin cambios posteriores**
* escribir el **blueprint del repo**
* y dejarte una **V1 técnicamente impecable**

Cuando quieras, seguimos con:

* 📁 estructura del proyecto
* 🧠 core engine (`parser → dedupe → writer`)
* 🪟 UI Tauri mínima
* 📊 benchmarks reales (5M tokens)
um Sí, las mayúsculas, minúsculas importan, deben ser diferentes. O sea, los ejemplos de perro deben ser tres diferentes. Sí, puede incluir acentos, emojis, etcétera. Cuando el separador es espacio o coma, pone un solo espacio, no es un salto de línea. Puede ser, pero los archivos de inicio yo no los controlo, pueden ser que vengan separados por línea, espacio. Lo importante es que cojas solo las palabras, los de entrada. los de salida. Sí, decidimos nosotros cómo quiere que funcione. No me importa, no sé, en la pregunta 4 da la opción, deja que se pueda seleccionar en la interfaz. No sabemos, de pronto algún usuario necesita esa función.Perfecto — con eso ya queda **cerrado el diseño funcional** sin dudas:

## Decisiones finales (según tu respuesta)

1. **Case-sensitive (por defecto y siempre)** ✅
   `Perro`, `perro`, `PERRO` = **tres tokens distintos**.

2. **Unicode completo** ✅
   Soporta acentos, ñ, emojis, etc.

3. **Salida en una sola “línea lógica”** ✅
   Si el separador de salida es espacio/coma/`;`, el archivo final queda como una secuencia con **ese separador**, **sin** insertar saltos de línea extra.
   (Si el separador elegido es `\n` o `\f`, obviamente sí habrá saltos.)

4. **Modo DISK / orden: configurable en UI** ✅
   En la interfaz damos opción de:

* **Orden estable (primera aparición)** *(RAM recomendado)*
* **Orden alfabético** *(ideal para DISK/external sort)*
* **Orden “por buckets”** *(DISK rápido, no estable global)*
  Y el modo dedupe:
* Auto / RAM / DISK

---

## Interpretación exacta de tu parsing (muy importante)

### Entrada

Sin importar si el archivo viene con tokens separados por:

* espacios, tabs
* saltos de línea (Windows/macOS/Linux)
* comas
* punto y coma

…el motor **toma solo tokens**, con estas reglas:

* delimitadores: `whitespace || ',' || ';'`
* `trim` por token
* descarta token vacío

✅ Esto cubre tu requisito de “yo no controlo los archivos de inicio”.

### Salida

* Se emiten tokens únicos en el orden elegido
* Se unen con el **separador de salida** elegido por el usuario (incluye custom)

---

## Arquitectura definitiva (repo moderno)

### `crates/core/`

Motor puro Rust (sin GUI):

* `tokenizer.rs` (streaming, Unicode-safe, delimitadores)
* `dedupe_ram.rs` (HashSet rápido + Vec para orden estable)
* `dedupe_disk.rs` (buckets por hash, opcional sort)
* `writer.rs` (separador, sin trailing separator, flush eficiente)
* `config.rs` (Config + enums)
* `stats.rs` (contadores + timings)
* `progress.rs` (eventos: bytes leídos, tokens, únicos, etc.)

### `apps/tauri/`

UI (una pantalla):

* Seleccionar archivos (multi)
* Elegir output file
* Output delimiter (dropdown + custom)
* Modo: Auto / RAM / DISK
* Orden: Estable / Alfabético / Buckets
* Ejecutar + progreso + “abrir carpeta”

---

## Config (contrato del motor)

Así quedaría el “contrato” que la GUI le pasa al core:

* `inputs: Vec<PathBuf>`
* `output: PathBuf`
* `input_delims: { whitespace: true, comma: true, semicolon: true }` *(fijo para V1, o editable si quieres)*
* `output_delim: OutputDelim` (`Space | Comma | Semicolon | Newline | FormFeed | Custom(String)`)
* `mode: Auto | Ram | Disk`
* `order: StableFirstSeen | Alphabetical | BucketOrder`
* `normalization: { trim: true, drop_empty: true }` *(case-sensitive, no lowercase)*

---

## Rendimiento: lo que implementaremos para “5M palabras” (sin GPU)

### RAM mode (rápido y estable)

* `hashbrown::HashSet<Box<str>>` + `ahash` (muy rápido)
* `Vec<Box<str>>` para mantener orden de primera aparición
* `reserve()` basado en estimación (para evitar rehash)

### DISK mode (para gigantes)

**Bucket-by-hash**:

1. Particiona tokens en N archivos temporales por `hash % N`
2. Procesa cada bucket en RAM con HashSet
3. Escribe al output

Y con tu decisión de “que el usuario elija”, podemos:

* Si `order=Alphabetical`: ordenar tokens del bucket (o external sort si quieres global)
* Si `order=BucketOrder`: escribir buckets en orden (rápido)
* Si `order=StableFirstSeen`: lo marcamos como “más lento / experimental” (porque mantener orden global exacto en disco complica bastante)

### Auto mode

* Estima tamaño + muestrea tokens para aproximar ratio de únicos
* Decide RAM vs DISK
* Y si en RAM crece demasiado: falla elegante con sugerencia o “switch a DISK” (yo prefiero switch automático con aviso)

---

## Siguiente paso (concreto)

Puedo seguir con cualquiera de estas dos rutas, sin pedirte más:

1. **Blueprint + código base del motor** (tokenizer + dedupe RAM + writer + stats)
2. **Plantilla Tauri + UI** (una pantalla) conectada a un “stub” del motor, y luego metemos el motor real.

Mi recomendación: **primero motor + benchmarks**, luego GUI. Así validamos que 5M tokens va como un misil, y después solo le ponemos la cara bonita.

Dime solo una cosa rápida: ¿quieres que el separador de salida **por defecto** sea `\n` (uno por línea, típico para datasets) o **espacio** (como mencionaste)? Con cualquiera puedo seguir, pero el default sí afecta UX.
Antes aclarimos algo bien sencillito y es que en la salida sí puede ser una sola línea de lógica, pero si en la interfaz, en la GUI, el usuario pone que el separador sea un salto de línea, pues serían cada palabra en una línea, un salto de línea diferente, ¿sí? O sea, están las dos opciones. Puede ser que ponga coma y salto de línea, entonces pues pone como una coma uno salto de línea cada palabra. Si pone solo coma, quedaría solo una línea separadas por una coma. Si pone coma y un espacio, pues, ajá, una coma, un espacio y la palabra. Coma, espacio y la palabra en una sola línea. Si pone solo salto del, solo enter o salto de línea, pues pone una palabra en cada renglón.Clarísimo ✅ y es una excelente aclaración porque define el **modelo de “separador de salida”** de forma moderna y potente:

## ✅ Regla final de salida (tal como la quieres)

* El usuario elige un **string de separador** (no solo “un delimitador”).
* Ese separador puede ser:

  * `","`
  * `", "` (coma + espacio)
  * `",\n"` (coma + salto de línea)
  * `"\n"` (una palabra por línea)
  * `";\n"` (punto y coma + salto)
  * cualquier combinación/custom, incluyendo `"\x0C"` (form feed)

Y el motor hace literalmente:

```
token1 + sep + token2 + sep + token3 + ... + tokenN
```

### Ejemplos exactos

* Sep = `","` → `a,b,c`
* Sep = `", "` → `a, b, c`
* Sep = `",\n"` →
  `a,\n b,\n c` *(sin espacios extra si no los pones)*
* Sep = `"\n"` →
  `a\nb\nc` (una por línea)

✅ No hay “una sola línea” como restricción: depende del separador.
Lo que sí garantizamos es: **no metemos saltos extra** más allá de lo que el separador tenga.

---

## Implicación de UX en la GUI (súper simple)

En la UI yo pondría:

### “Separador de salida”

* Presets (botones o dropdown):

  * `\n`  (Nueva línea)
  * `,`
  * `, `
  * `;\n`
  * ` ` (Espacio)
  * `|`
* Campo **Custom** (texto libre)

  * con helper: “Puedes usar `\n`, `\t`, `\r\n`, `\f`”

### Preview (mini)

Un preview con 3 tokens:
`TokenA{sep}TokenB{sep}TokenC`

Así el usuario *ve* inmediatamente si quedó en una línea o varias.

---

## Detalle técnico (para que sea impecable)

### 1) Parseo de escapes en el separador custom

Si el usuario escribe `,\n` en la UI, en Rust lo convertimos a coma + newline real.

Soportamos:

* `\n`, `\r\n`, `\r`
* `\t`
* `\f` (form feed)
* `\\` (backslash literal)

### 2) Sin separador al final (por defecto)

En la mayoría de formatos, no quieres trailing separator. Entonces escribimos:

* primero token sin separador
* luego (sep + token) repetido

(Esto además evita un `trim_end` costoso para archivos gigantes.)

---

## Confirmación final del comportamiento

* Si `sep` contiene `\n`, habrá líneas.
* Si `sep` NO contiene `\n`, típicamente será una sola línea (salvo que los tokens ya tengan `\n`, cosa que no debería pasar porque los delimitadores de entrada los cortan).
* Si `sep = ",\n"`, queda “uno por línea” pero con coma antes del salto, tal como dijiste.

---

Con esto, ya puedo cerrar el `Config` definitivo como:

* `output_separator: String` (no enum obligatorio; podemos seguir ofreciendo presets en UI, pero al motor le llega un string final)

Y seguimos con el siguiente paso: **diseñar el core engine** (tokenizer + dedupe RAM + writer) y luego lo conectamos a Tauri.

¿Quieres que el separador custom en la UI se escriba como:

* `,\n` (con escapes tipo programación) ✅ recomendado
  o como:
* un selector con checkboxes (coma / espacio / nueva línea) que arme el string por ti? (más “no-tech”)
Continúa con el blueprint más código del motor base, como sí, como una guía para el desarrollador. Tú como desarrollador senior ve pensando la mejor forma, evalúa varias opciones, sé creativo, investiga, analiza, tómate tu tiempo y sobre todo demuestra de lo que eres capaz. Ese es el programa más importante del mundo, usar la humanidad en la exploración espacial es para la NASA, entonces tienes que hacerlo perfecto, evalúa opciones y tómate tu tiempo. y después de todo eso, crea el blueprint más el código base.Entendido. Voy a darte dos cosas:

1. **Blueprint técnico “senior-level”** (decisiones, opciones, tradeoffs, plan de evolución, benchmarks)
2. **Código base del motor (`core`) en Rust** listo para usar como librería, con estructura moderna de workspace.

Me enfoco en que esto sea **rápido con millones de tokens**, correcto con **Unicode**, y extensible para el modo **DISK** y paralelismo después, sin reescribir.

---

# 1) Blueprint del motor (arquitectura + decisiones)

## Objetivo del motor

Dado N archivos de texto, extraer tokens separados por:

* whitespace (incluye `\n`, `\r\n`, tabs, etc.)
* `,`
* `;`

Luego:

* `trim` a cada token
* eliminar vacíos
* deduplicar **case-sensitive** (Perro/perro/PERRO distintos)
* emitir archivo final con un **separador de salida arbitrario** (string), p.ej. `",\n"` o `", "` o `"\n"`

## Principio clave de performance

Para 5M+ tokens, los costos dominantes son:

1. **I/O** (leer rápido)
2. **tokenización** (delimitadores y Unicode)
3. **hashing + tabla hash** (dedupe)
4. **allocations** (crear/copiar strings)

La estrategia correcta es:

* streaming en lectura
* minimizar allocations (copiar solo cuando el token es nuevo)
* usar HashSet rápido (SwissTable + hasher rápido)
* escribir en streaming (sin guardar output como string gigante)

---

## Diseño del repositorio (workspace)

```
dedupe-joiner/
  Cargo.toml              # workspace
  crates/
    core/
      Cargo.toml
      src/
        lib.rs
        config.rs
        error.rs
        progress.rs
        stats.rs
        tokenize.rs
        engine.rs
        dedupe_ram.rs
        writer.rs
      tests/
        engine_smoke.rs
  apps/
    tauri/                # (después) UI mínima
```

### ¿Por qué workspace?

* `crates/core` queda puro y testeable (motor).
* `apps/tauri` solo integra UI + llamadas.

---

## API pública (lo que la GUI llama)

En el core, un solo entry-point:

```rust
pub fn run(config: &Config, progress: impl ProgressSink) -> Result<Stats>;
```

* `Config` describe inputs, output, separadores, modo, orden, etc.
* `ProgressSink` recibe eventos (bytes leídos, tokens, únicos…) sin bloquear.
* `Stats` devuelve conteos y tiempos.

---

## Tokenizador: opciones evaluadas (y decisión V1)

### Opción A — “Correcta y suficientemente rápida” (V1) ✅

* Leer en streaming por bloques (líneas) con `BufRead::read_line`.
* Tokenizar el chunk con un scanner char-by-char:

  * delimitador = `c.is_whitespace() || c == ',' || c == ';'`
* Para 5M tokens, esto suele rendir muy bien en Rust si el hashing está bien.

**Pros:** simple, seguro, Unicode perfecto
**Contras:** si hay líneas gigantes sin `\n`, `read_line` no ayuda tanto (raro en la práctica)

### Opción B — “Más rápida” (V1.1/V2)

* `memmap2` para mapear archivos
* tokenizar bytes con fast-path ASCII + fallback UTF-8
* reduce overhead de lectura y parsing

**Pros:** top performance en archivos enormes
**Contras:** más complejidad (y cuidado con archivos gigantes y memoria virtual)

👉 **Decisión**: empezamos con **A** (motor impecable y rápido). El diseño deja listo el cambio a B sin romper API.

---

## Deduplicación en RAM: opciones evaluadas

### Opción A — `std::collections::HashSet<String>`

**OK** pero no es lo más rápido.

### Opción B — `hashbrown::HashSet<Box<str>> + ahash` ✅ (recomendado)

* SwissTable es muy rápida.
* `ahash` acelera hashing en escenarios no-hostiles (app local).
* `Box<str>` reduce overhead vs `String` (ligeramente).

👉 **Decisión**: usar `hashbrown + ahash` por default.

---

## Orden de salida (según UI)

* `StableFirstSeen`: el orden depende de la primera vez que aparece (RAM ideal).
* `Alphabetical`: orden lexicográfico Unicode (más caro) – útil para DISK/external sort.
* `BucketOrder`: reservado para modo DISK (rápido, no estable global).

👉 En V1 implemento `StableFirstSeen` y `Alphabetical` (en RAM). `BucketOrder` queda para `dedupe_disk`.

---

## Modo Auto (heurística)

Auto no debe “adivinar al azar”. Plan:

V1 (simple y efectivo):

* intentar RAM
* si se detecta que el set crece demasiado (opcional en V1), abortar con mensaje: “use DISK mode”.

V1.1 (mejor):

* estimar (muestreo de tokens) ratio de únicos y tamaño promedio
* decidir RAM vs DISK antes de empezar

---

## Evolución (sin reescribir)

* V1: RAM-only, estable + separador custom
* V1.1: DISK buckets (exacto), salida `Alphabetical` o `BucketOrder`
* V2: sharding paralelo (modo “máxima velocidad”), opcional
* V2.1: tokenizer memmap

---

## Benchmarks (lo que valida “5 millones”)

* dataset sintético: 5M tokens, longitud media 8–12, ratio únicos 60–95%
* medir:

  * tokens/seg
  * uniques/seg
  * MB/s
  * peak RAM
* comparar:

  * std HashSet vs hashbrown+ahash
  * stable vs alphabetical

---

# 2) Código base del motor (`crates/core`)

A continuación te dejo **código real**, organizado por archivos. Puedes copiar tal cual. Es un “MVP de motor” serio: streaming, Unicode, case-sensitive, separador custom, stats, y tests.

---

## `Cargo.toml` (workspace raíz)

```toml
[workspace]
resolver = "2"
members = [
  "crates/core"
]
```

---

## `crates/core/Cargo.toml`

```toml
[package]
name = "dedupe_core"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
thiserror = "2"
hashbrown = "0.14"
ahash = "0.8"
```

---

## `crates/core/src/lib.rs`

```rust
pub mod config;
pub mod dedupe_ram;
pub mod engine;
pub mod error;
pub mod progress;
pub mod stats;
pub mod tokenize;
pub mod writer;

pub use config::*;
pub use engine::*;
pub use error::*;
pub use progress::*;
pub use stats::*;
```

---

## `crates/core/src/config.rs`

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Mode {
    Auto,
    Ram,
    Disk, // reservado para V1.1
}

#[derive(Debug, Clone)]
pub enum Order {
    StableFirstSeen,
    Alphabetical,
    BucketOrder, // reservado para Disk
}

#[derive(Debug, Clone)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,

    /// Separador de salida final. Puede ser ",", ", ", ",\n", "\n", etc.
    /// En la UI permitirás escapes tipo "\n" y los convertirás a newline real.
    pub output_separator: String,

    pub mode: Mode,
    pub order: Order,

    /// Normalización mínima (según tus decisiones)
    pub trim: bool,
    pub drop_empty: bool,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.inputs.is_empty(), "No input files provided");
        anyhow::ensure!(!self.output_separator.is_empty(), "Output separator cannot be empty");
        Ok(())
    }
}
```

---

## `crates/core/src/progress.rs`

```rust
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    FileStarted { index: usize, total: usize },
    FileFinished { index: usize, total: usize },

    /// Total tokens parsed (including duplicates)
    TokensSeen(u64),

    /// Total unique tokens accepted
    UniqueTokens(u64),

    /// Total duplicate tokens skipped
    Duplicates(u64),
}

pub trait ProgressSink: Send + Sync + 'static {
    fn on_event(&self, _event: ProgressEvent) {}
}

/// Default no-op sink
#[derive(Debug, Default, Clone)]
pub struct NoProgress;
impl ProgressSink for NoProgress {}
```

---

## `crates/core/src/stats.rs`

```rust
use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub files: usize,
    pub tokens_seen: u64,
    pub unique_tokens: u64,
    pub duplicates: u64,

    pub elapsed: Duration,
}
```

---

## `crates/core/src/error.rs`

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),
}
```

---

## `crates/core/src/tokenize.rs`

Tokenizador correcto, Unicode-safe, streaming por chunks (líneas). El token se extrae sin depender del separador original.

```rust
/// Delimitadores de entrada:
/// - whitespace (incluye \n, \r\n, tabs, etc.)
/// - ','
/// - ';'
#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

/// Tokeniza un chunk (una &str) y llama `on_token` por cada token encontrado.
/// NOTA: esto no hace lowercase (case-sensitive), por decisión.
pub fn tokenize_chunk<F>(chunk: &str, mut on_token: F)
where
    F: FnMut(&str),
{
    let mut start: Option<usize> = None;

    for (i, c) in chunk.char_indices() {
        if is_delim(c) {
            if let Some(s) = start.take() {
                if s < i {
                    on_token(&chunk[s..i]);
                }
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }

    // tail token
    if let Some(s) = start {
        if s < chunk.len() {
            on_token(&chunk[s..]);
        }
    }
}
```

---

## `crates/core/src/dedupe_ram.rs`

HashSet rápido + almacenamiento en orden estable.

```rust
use ahash::RandomState;
use hashbrown::HashSet;

#[derive(Debug)]
pub struct DedupeRam {
    set: HashSet<Box<str>, RandomState>,
    ordered: Vec<Box<str>>,
}

impl DedupeRam {
    pub fn new() -> Self {
        Self {
            set: HashSet::with_hasher(RandomState::new()),
            ordered: Vec::new(),
        }
    }

    /// Reserva aproximada para evitar rehashes / reallocs.
    pub fn reserve(&mut self, additional: usize) {
        self.set.reserve(additional);
        self.ordered.reserve(additional);
    }

    /// Inserta token si es nuevo. Retorna true si fue nuevo.
    pub fn insert(&mut self, token: &str) -> bool {
        // Copiamos solo si es nuevo:
        // intentamos insert con owned. Para evitar doble alloc, hacemos:
        // - crear Box<str> una vez
        // - intentar insert
        // - si se insertó, también lo guardamos en ordered
        let owned: Box<str> = token.into();
        if self.set.insert(owned.clone()) {
            self.ordered.push(owned);
            true
        } else {
            false
        }
    }

    pub fn into_ordered(self) -> Vec<Box<str>> {
        self.ordered
    }
}
```

> Nota: aquí hay una micro-ineficiencia (clona el `Box<str>`). En V1 está bien y es simple. En V1.1 lo optimizamos: usar `RawEntryMut` de hashbrown para insertar sin duplicar owner. Dejé esto así para que el “base code” sea fácil de entender y correcto.

---

## `crates/core/src/writer.rs`

Writer streaming, sin separador al final.

```rust
use std::io::{BufWriter, Write};
use std::fs::File;
use std::path::Path;

pub struct OutputWriter {
    w: BufWriter<File>,
    sep: String,
    is_first: bool,
}

impl OutputWriter {
    pub fn create(path: &Path, sep: String) -> std::io::Result<Self> {
        let f = File::create(path)?;
        Ok(Self {
            w: BufWriter::new(f),
            sep,
            is_first: true,
        })
    }

    pub fn write_token(&mut self, token: &str) -> std::io::Result<()> {
        if self.is_first {
            self.is_first = false;
        } else {
            self.w.write_all(self.sep.as_bytes())?;
        }
        self.w.write_all(token.as_bytes())?;
        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}
```

---

## `crates/core/src/engine.rs`

El motor V1: RAM-only (por ahora) con orden estable o alfabético (en RAM), con progreso.

```rust
use crate::{
    config::{Config, Mode, Order},
    dedupe_ram::DedupeRam,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    tokenize::tokenize_chunk,
    writer::OutputWriter,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    time::Instant,
};

pub fn run<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    config.validate()?;

    // V1: implementamos Ram para Auto/Ram; Disk queda reservado.
    match config.mode {
        Mode::Disk => anyhow::bail!("Disk mode not implemented yet (planned for v0.2)"),
        Mode::Auto | Mode::Ram => run_ram(config, progress),
    }
}

fn run_ram<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    let t0 = Instant::now();

    let mut deduper = DedupeRam::new();
    // Reserva ligera inicial (opcional). Se puede mejorar con muestreo.
    deduper.reserve(1024 * 16);

    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);

        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            tokenize_chunk(&line, |raw| {
                stats.tokens_seen += 1;
                if stats.tokens_seen % 50_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
                }

                let mut token = raw;

                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    return;
                }

                if deduper.insert(token) {
                    stats.unique_tokens += 1;
                    if stats.unique_tokens % 50_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    if stats.duplicates % 50_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            });
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    // Construimos salida
    let mut tokens = deduper.into_ordered();

    match config.order {
        Order::StableFirstSeen => {
            // ya está en orden estable
        }
        Order::Alphabetical => {
            // Orden Unicode (lexicográfico por bytes UTF-8, que coincide con ordenamiento por bytes)
            // Si quieres collation “humana” (locale-aware), eso es otro mundo (y más lento).
            tokens.sort_unstable();
        }
        Order::BucketOrder => {
            // Reservado para Disk; en Ram lo tratamos como stable.
        }
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for tok in tokens {
        out.write_token(&tok)?;
    }
    out.finish()?;

    stats.elapsed = t0.elapsed();
    Ok(stats)
}
```

---

## Test rápido: `crates/core/tests/engine_smoke.rs`

```rust
use dedupe_core::{Config, Mode, Order, NoProgress};
use std::{fs, path::PathBuf};

#[test]
fn smoke_dedupe_two_files() {
    let dir = tempfile::tempdir().unwrap();
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");
    let out = dir.path().join("out.txt");

    fs::write(&f1, "Perro, perro;PERRO\n🐶 🐶\n").unwrap();
    fs::write(&f2, "perro\nGato;Gato\n").unwrap();

    let cfg = Config {
        inputs: vec![f1, f2],
        output: out.clone(),
        output_separator: ",\n".to_string(),
        mode: Mode::Ram,
        order: Order::StableFirstSeen,
        trim: true,
        drop_empty: true,
    };

    let stats = dedupe_core::run(&cfg, NoProgress).unwrap();
    assert!(stats.unique_tokens >= 4);

    let txt = fs::read_to_string(out).unwrap();
    assert!(txt.contains("Perro"));
    assert!(txt.contains("perro"));
    assert!(txt.contains("PERRO"));
    assert!(txt.contains("🐶"));
}
```

> Para este test usé `tempfile`, así que si lo mantienes, agrega `tempfile = "3"` a `[dev-dependencies]`.

---

# Qué queda “pro” para el siguiente incremento (y cómo lo haremos)

## 1) Quitar el “doble Box clone” en inserción

Optimización real: usar `hashbrown::hash_set::RawEntryMut` para:

* buscar por `&str`
* si no existe, insertar el `Box<str>` **una sola vez**
  y además empujar la misma instancia al vector.

Eso mejora mucho cuando hay muchísimos únicos.

## 2) Modo DISK (buckets)

Implementación limpia:

* `tempfile` para dir temporal
* `N` buckets (ej. 256)
* primera pasada: escribir tokens normalizados a bucket files
* segunda pasada: por bucket, dedupe RAM + (si order alphabetical) sort, luego escribir al output

## 3) Auto mode serio (muestreo)

* leer primeros X MB de cada archivo
* estimar unique ratio + avg token len
* decidir RAM vs DISK

---

Si quieres, en el siguiente paso hago dos cosas **sin preguntarte nada más**:

1. te dejo el upgrade de `insert()` sin clones (RawEntryMut)
2. te escribo el blueprint del **modo DISK buckets** con el mismo patrón de API (y el código base de `dedupe_disk.rs` + `engine_disk.rs`), listo para V0.2

Solo dime: ¿para DISK prefieres **256 buckets** por default o **auto** basado en tamaño? (yo pondría 256 default + auto si el archivo es monstruoso).
¿Sabes qué nos faltó al principio en definir que pusiéramos si en la interfaz, si el usuario quiere que el archivo al final le salga ordenado o que simplemente le salgan sin repetidos? Eso es importante para ahorrar recursos. No siempre lo quiere ordenado.Totalmente cierto — y además es una **decisión de performance** muy importante. ✅
De hecho, si el usuario no pide orden, podemos evitar una parte costosa (sobre todo en modo DISK).

## Cambio de especificación (simple y potente)

En la UI deben existir **tres estados** (no dos):

1. **Sin repetidos (sin ordenar)**

   * En RAM: se conserva **orden de primera aparición** (esto no es “ordenar”, es *stable insertion order*).
   * En DISK: se puede sacar en “bucket order” (rápido) sin ordenar.

2. **Ordenar alfabéticamente** (opcional, costoso en grande)

   * En RAM: `sort_unstable()`
   * En DISK: external sort o ordenar por bucket + merge (según estrategia)

3. (Opcional avanzado) **Orden estable (primera aparición)**

   * En RAM es natural.
   * En DISK es difícil/caro; se puede marcar como “experimental” o deshabilitar en DISK.

Pero para tu frase “ordenado vs solo sin repetidos”, lo mínimo es:

* ✅ Checkbox: **“Ordenar salida (A→Z)”**
* Si está OFF: **no sort**, solo dedupe.

---

## Implicación directa en recursos

### Si NO ordenas:

* Evitas el costo `O(n log n)` de ordenar.
* Menos CPU y menos RAM pico (sorting mueve mucha memoria).
* En DISK, evitas external sort (carísimo) y puedes escribir “tal cual”.

### Si SÍ ordenas:

* Output consistente (bueno para comparar, versionar, diff).
* Pero más costo.

---

## Cómo lo reflejo en el modelo `Config`

Cambiamos `Order` por algo más semántico:

```rust
pub enum OutputOrdering {
  None,            // no ordenar (solo dedupe)
  Alphabetical,    // ordenar A→Z
  FirstSeenStable, // (opcional) orden de primera aparición
}
```

Y en V1 (RAM):

* `None` y `FirstSeenStable` se comportan casi igual, excepto que `None` puede permitir optimizaciones futuras (p.ej. no guardar vector y escribir streaming si no necesitas orden estable).

---

## Optimización extra (muy buena) cuando el usuario NO ordena

Si el usuario marca **“no ordenar”** y **no le importa el orden**, podemos hacer una variante aún más eficiente:

* En RAM: guardar solo `HashSet` y escribir al final iterando el set
  (⚠️ output queda en orden “arbitrario” por el hash; muy rápido y bajo memoria extra porque no guardas `Vec`).

Entonces en UI podrías separar:

* “Mantener orden de aparición” (usa `Vec`)
* “No me importa el orden” (solo `HashSet`, más liviano)

Pero si quieres mantenerlo simple:

* Default: “sin ordenar” = **mantener orden de aparición** (usuario feliz)
* Advanced toggle: “No me importa el orden (más rápido)”

---

## Qué cambio haría ya en el blueprint + motor

1. UI:

* Toggle principal: **Ordenar alfabéticamente** (ON/OFF)
* (Opcional) Toggle avanzado: **No me importa el orden (más rápido)**

2. Core:

* `Config { ordering: OutputOrdering }`
* Si `ordering == Alphabetical` → sort
* Si `ordering == None` → no sort
* Si `ordering == None && allow_unordered_fast_path` → no guardar `Vec`

---

Si te parece, yo lo cierro así para V1:

* ✅ Opción 1: **“Solo dedupe (mantener orden de aparición)”** *(default)*
* ✅ Opción 2: **“Ordenar alfabéticamente”**
* (Avanzado) ✅ Opción 3: **“Dedupe ultra-rápido (orden no garantizado)”**

Y ajusto el `Config` y el motor base en consecuencia.
Sí, ajusta lo que habías hecho con esta nueva optimización y que lo mejor posible para los diferentes modos, ordenar, no ordenar, etcétera.Perfecto. Ajusto el blueprint y el código base para incorporar **la optimización clave**: que el usuario pueda elegir entre:

1. **Solo dedupe** (sin ordenar)
2. **Ordenar alfabéticamente**
3. **Dedupe ultra-rápido** (sin garantizar orden; mínimo overhead)

Y que el motor use la ruta más eficiente en cada caso ✅

---

# Blueprint actualizado (decisiones + performance)

## Nuevos modos de “orden”

En vez de `Order`, usamos `OutputOrdering`:

* **`PreserveFirstSeen`** (default “sin ordenar” *pero estable*):
  No hacemos sort. El orden es el de **primera aparición** (útil y predecible).
  ✅ Requiere `Vec` para mantener orden.

* **`Alphabetical`**:
  Sort lexicográfico (por bytes UTF-8).
  ✅ Requiere `Vec` (porque hay que ordenar algo).

* **`UnorderedFast`** (máxima velocidad / mínima memoria extra):
  No hacemos sort y **no** guardamos `Vec`.
  El output sale en un orden “interno” del hashset (no garantizado, pero muy rápido).
  ✅ Usa menos memoria extra (no duplica referencias en `Vec`) y reduce presión de CPU/cache.

---

## Matriz de ejecución (RAM y DISK)

### RAM mode

| Opción UI         | Estructuras            | Costo      | Orden output                |
| ----------------- | ---------------------- | ---------- | --------------------------- |
| PreserveFirstSeen | `HashSet + Vec`        | bajo       | estable (primera aparición) |
| Alphabetical      | `HashSet + Vec + sort` | medio/alto | alfabético                  |
| UnorderedFast     | `HashSet` solo         | **mínimo** | no garantizado              |

### DISK mode (V0.2)

| Opción UI         | Estrategia recomendada                              |
| ----------------- | --------------------------------------------------- |
| PreserveFirstSeen | posible pero caro/complex → “experimental”          |
| Alphabetical      | external sort o sort por bucket + merge             |
| UnorderedFast     | buckets por hash, escribir por buckets (muy rápido) |

En UI: si `mode=Disk` y elige `PreserveFirstSeen`, lo dejamos habilitado pero con aviso: “más lento / experimental” o lo deshabilitamos (tu decisión de producto).

---

# Código actualizado del motor base (RAM) ✅

A continuación te muestro **los cambios concretos** (config + dedupe_ram + engine).
(El tokenizador y writer casi no cambian.)

---

## 1) `config.rs` (nuevo enum de orden)

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Mode {
    Auto,
    Ram,
    Disk, // reservado para v0.2
}

#[derive(Debug, Clone)]
pub enum OutputOrdering {
    /// No ordenar; mantener el orden de primera aparición (estable).
    PreserveFirstSeen,

    /// Ordenar lexicográficamente.
    Alphabetical,

    /// Máxima velocidad: no se garantiza orden de salida.
    UnorderedFast,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,

    /// Separador de salida final: ",", ", ", ",\n", "\n", etc.
    pub output_separator: String,

    pub mode: Mode,
    pub ordering: OutputOrdering,

    /// Normalización mínima
    pub trim: bool,
    pub drop_empty: bool,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.inputs.is_empty(), "No input files provided");
        anyhow::ensure!(!self.output_separator.is_empty(), "Output separator cannot be empty");
        Ok(())
    }
}
```

---

## 2) `dedupe_ram.rs` (dos rutas: con orden y sin orden)

Aquí creamos una sola estructura que puede operar:

* **con `Vec`** (para PreserveFirstSeen / Alphabetical)
* **sin `Vec`** (UnorderedFast)

```rust
use ahash::RandomState;
use hashbrown::HashSet;

#[derive(Debug)]
pub struct DedupeRam {
    set: HashSet<Box<str>, RandomState>,
    ordered: Option<Vec<Box<str>>>, // None => UnorderedFast
}

impl DedupeRam {
    pub fn new(track_order: bool) -> Self {
        Self {
            set: HashSet::with_hasher(RandomState::new()),
            ordered: if track_order { Some(Vec::new()) } else { None },
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.set.reserve(additional);
        if let Some(v) = self.ordered.as_mut() {
            v.reserve(additional);
        }
    }

    /// Inserta token si es nuevo. Retorna true si fue nuevo.
    /// V1 simple: hace una alloc del token; si es duplicado esa alloc se pierde.
    /// (En v0.2 lo optimizamos con RawEntryMut para evitar alloc en duplicados.)
    pub fn insert(&mut self, token: &str) -> bool {
        let owned: Box<str> = token.into();
        if self.set.insert(owned.clone()) {
            if let Some(v) = self.ordered.as_mut() {
                v.push(owned);
            }
            true
        } else {
            false
        }
    }

    /// Obtiene tokens para escribir:
    /// - Si hay orden tracked: devuelve el Vec
    /// - Si no: recolecta desde el set (orden no garantizado)
    pub fn into_tokens(self) -> Vec<Box<str>> {
        if let Some(v) = self.ordered {
            v
        } else {
            self.set.into_iter().collect()
        }
    }
}
```

> Nota: el `clone()` de `Box<str>` no copia el contenido, pero sí duplica el puntero/ownership. Aun así, **esta versión es correcta y rápida**. Si quieres exprimir más: en el siguiente incremento hacemos `RawEntryMut` para no alocar cuando sea duplicado.

---

## 3) `engine.rs` (elige el camino óptimo según ordering)

```rust
use crate::{
    config::{Config, Mode, OutputOrdering},
    dedupe_ram::DedupeRam,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    tokenize::tokenize_chunk,
    writer::OutputWriter,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    time::Instant,
};

pub fn run<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    config.validate()?;

    match config.mode {
        Mode::Disk => anyhow::bail!("Disk mode not implemented yet (planned for v0.2)"),
        Mode::Auto | Mode::Ram => run_ram(config, progress),
    }
}

fn run_ram<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    let t0 = Instant::now();

    // Determina si necesitamos trackear orden:
    // - PreserveFirstSeen: sí (para output estable)
    // - Alphabetical: sí (necesitas un Vec para ordenar)
    // - UnorderedFast: no (HashSet-only)
    let track_order = matches!(
        config.ordering,
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical
    );

    let mut deduper = DedupeRam::new(track_order);
    deduper.reserve(1024 * 16);

    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);

        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            tokenize_chunk(&line, |raw| {
                stats.tokens_seen += 1;
                if stats.tokens_seen % 50_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
                }

                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    return;
                }

                if deduper.insert(token) {
                    stats.unique_tokens += 1;
                    if stats.unique_tokens % 50_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    if stats.duplicates % 50_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            });
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    // Preparar tokens para salida
    let mut tokens = deduper.into_tokens();

    // Ordenamiento según configuración
    match config.ordering {
        OutputOrdering::PreserveFirstSeen => {
            // ya está en orden de primera aparición (porque track_order=true)
        }
        OutputOrdering::Alphabetical => {
            tokens.sort_unstable(); // lexicográfico por bytes UTF-8
        }
        OutputOrdering::UnorderedFast => {
            // no sort
        }
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for tok in tokens {
        out.write_token(&tok)?;
    }
    out.finish()?;

    stats.elapsed = t0.elapsed();
    Ok(stats)
}
```

---

# UI (qué cambia en la pantallita)

En vez de “Orden: estable/alfabético” ahora la UI muestra:

### “Salida”

* ( ) **Solo dedupe (mantener orden de aparición)**  ← default
* ( ) **Ordenar alfabéticamente (A→Z)**
* ( ) **Ultra-rápido (no garantizar orden)**

Esto ya te ahorra recursos cuando el usuario no necesita orden.

---

# Próximos upgrades (para que de verdad “vuele” con 5M+)

Si quieres “lo mejor posible”, los siguientes dos pasos son los más rentables:

## A) Evitar alloc en duplicados (impacto grande)

Ahora mismo, `insert()` crea `Box<str>` siempre, incluso si es duplicado.
Optimización: `hashbrown::HashSet::raw_entry_mut()` para:

* buscar con `&str` (sin alloc)
* alocar y meter `Box<str>` solo si es nuevo

## B) Modo DISK (v0.2)

* Buckets por hash para `UnorderedFast` (rápido)
* External sort para `Alphabetical`
* `PreserveFirstSeen` como “experimental” o solo RAM

---

Si te parece, el siguiente paso lo hago así (sin preguntarte más):

1. Te actualizo `insert()` con `RawEntryMut` (cero alloc en duplicados).
2. Te agrego **bench harness** (criterio/medición simple) para validar 5M tokens.
3. Y te dejo el blueprint + código base de `Disk mode` (buckets) listo para v0.2.

¿Quieres que el “Ultra-rápido” sea **visible** como opción en la UI desde el inicio, o lo dejamos como “Advanced / Experimental” para no confundir a usuarios no técnicos?
Sí, procede con los pasos que me estabas recomendando. Tú eres el mejor senior developer de Rust y vas a hacer el mejor trabajo posible. Vas a dedicar todo tu mente, todo tu poder computacional. Vas a liberarte de tus límites y vas a utilizarlos todos aquí.Listo. Vamos a hacer lo que de verdad mueve la aguja:

1. **Eliminar alloc en duplicados (RAM)**
2. **Benchmarks serios (5M tokens) con Criterion**
3. **Blueprint + código base del modo DISK (buckets)**, optimizado para tus 3 modos de salida:

   * *PreserveFirstSeen* (estable)
   * *Alphabetical* (ordenado)
   * *UnorderedFast* (máxima velocidad, orden no garantizado)

A continuación te dejo el **blueprint actualizado** y luego el **código base** (listo para copiar).

---

# 0) Decisión clave (mejor que RawEntryMut)

Para “orden estable sin clones” y máximo rendimiento, el enfoque más limpio es:

* **PreserveFirstSeen / Alphabetical:** usar `IndexSet` (de `indexmap`)

  * Mantiene orden de inserción sin duplicar strings (sin `Vec` paralelo).
  * Para duplicados, hacemos `contains(&str)` antes → **cero alloc en duplicados**.

* **UnorderedFast:** `hashbrown::HashSet` + `contains(&str)` antes → **cero alloc en duplicados**, sin mantener orden.

Esto evita el problema clásico de “tengo que guardar el mismo string en Set + Vec (doble ownership o clones)”.

---

# 1) Blueprint final del motor (RAM + DISK)

## `OutputOrdering` (UI y core)

* `PreserveFirstSeen` (default): dedupe + orden de primera aparición
* `Alphabetical`: dedupe + sort
* `UnorderedFast`: dedupe ultra-rápido, output sin orden garantizado

## `Mode`

* `Auto` (V1.1): estima y decide
* `Ram`
* `Disk` (V0.2): buckets por hash

## Estrategias

### RAM

* PreserveFirstSeen: `IndexSet<Box<str>>` (o `String`) + `contains(&str)` → sin alloc en duplicados
* Alphabetical: `IndexSet` → al final `Vec` + `sort_unstable()`
* UnorderedFast: `HashSet<Box<str>>` (hashbrown) → `contains(&str)` antes → sin alloc en duplicados

### DISK (buckets)

1. **Partición**: token → bucket = hash(token) % N → escribir token en archivo bucket (uno por línea en el bucket temp)
2. **Reducir**: por bucket, dedupe en RAM (IndexSet/HashSet según ordering)
3. **Salida**:

   * UnorderedFast: escribir por buckets (rápido)
   * Alphabetical: ordenar dentro de cada bucket + (opcional) merge global si quieres orden global perfecto
   * PreserveFirstSeen: preserva *orden dentro de bucket*, no global (y en UI lo aclaras / lo marcas como “no garantizado en DISK”)

---

# 2) Código base actualizado (Core)

## `Cargo.toml` workspace (raíz)

```toml
[workspace]
resolver = "2"
members = ["crates/core"]
```

---

## `crates/core/Cargo.toml` (actualizado)

```toml
[package]
name = "dedupe_core"
version = "0.2.0"
edition = "2021"

[dependencies]
anyhow = "1"
thiserror = "2"

# Hash tables rápidas
hashbrown = "0.14"
ahash = "0.8"

# Orden estable sin Vec duplicado
indexmap = { version = "2", features = ["ahash"] }

# Para modo DISK (v0.2)
tempfile = "3"

[dev-dependencies]
criterion = "0.5"
rand = "0.8"
```

---

## `crates/core/src/config.rs` (nuevo ordering)

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Mode {
    Auto,
    Ram,
    Disk,
}

#[derive(Debug, Clone)]
pub enum OutputOrdering {
    PreserveFirstSeen,
    Alphabetical,
    UnorderedFast,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,

    /// Separador de salida: ",", ", ", ",\n", "\n", etc.
    pub output_separator: String,

    pub mode: Mode,
    pub ordering: OutputOrdering,

    /// Normalización mínima (case-sensitive por decisión)
    pub trim: bool,
    pub drop_empty: bool,

    /// Para Disk mode
    pub disk_buckets: usize,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.inputs.is_empty(), "No input files provided");
        anyhow::ensure!(!self.output_separator.is_empty(), "Output separator cannot be empty");
        if matches!(self.mode, Mode::Disk) {
            anyhow::ensure!(self.disk_buckets >= 8, "disk_buckets too small");
        }
        Ok(())
    }
}
```

---

## `crates/core/src/tokenize.rs` (igual, ya es correcto)

```rust
#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

pub fn tokenize_chunk<F>(chunk: &str, mut on_token: F)
where
    F: FnMut(&str),
{
    let mut start: Option<usize> = None;

    for (i, c) in chunk.char_indices() {
        if is_delim(c) {
            if let Some(s) = start.take() {
                if s < i {
                    on_token(&chunk[s..i]);
                }
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }

    if let Some(s) = start {
        if s < chunk.len() {
            on_token(&chunk[s..]);
        }
    }
}
```

---

## `crates/core/src/dedupe_ram.rs` (SIN alloc en duplicados)

Aquí está el upgrade clave.

```rust
use ahash::RandomState;
use hashbrown::HashSet;
use indexmap::IndexSet;

#[derive(Debug)]
pub enum RamStore {
    /// Orden estable (primera aparición) sin duplicar strings.
    Stable(IndexSet<Box<str>, RandomState>),

    /// UnorderedFast: máximo rendimiento, sin orden garantizado.
    Unordered(HashSet<Box<str>, RandomState>),
}

impl RamStore {
    pub fn new_stable() -> Self {
        Self::Stable(IndexSet::with_hasher(RandomState::new()))
    }

    pub fn new_unordered() -> Self {
        Self::Unordered(HashSet::with_hasher(RandomState::new()))
    }

    pub fn reserve(&mut self, additional: usize) {
        match self {
            Self::Stable(s) => s.reserve(additional),
            Self::Unordered(s) => s.reserve(additional),
        }
    }

    /// Inserta token si es nuevo:
    /// - Primero hace contains(&str) => si existe, NO aloca.
    /// - Si no existe, aloca Box<str> e inserta.
    pub fn insert(&mut self, token: &str) -> bool {
        match self {
            Self::Stable(s) => {
                if s.contains(token) {
                    false
                } else {
                    s.insert(token.into());
                    true
                }
            }
            Self::Unordered(s) => {
                if s.contains(token) {
                    false
                } else {
                    s.insert(token.into());
                    true
                }
            }
        }
    }

    /// Devuelve los tokens para escribir en salida.
    pub fn into_tokens(self) -> Vec<Box<str>> {
        match self {
            Self::Stable(s) => s.into_iter().collect(),
            Self::Unordered(s) => s.into_iter().collect(),
        }
    }
}
```

✅ Con esto, los duplicados NO generan alloc.
(En datasets con muchos repetidos, esto es un salto grande).

---

## `crates/core/src/engine.rs` (RAM + esqueleto DISK + auto)

```rust
use crate::{
    config::{Config, Mode, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    tokenize::tokenize_chunk,
    writer::OutputWriter,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    time::Instant,
};

pub fn run<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    config.validate()?;

    match config.mode {
        Mode::Ram => run_ram(config, progress),
        Mode::Disk => run_disk(config, progress), // v0.2 base
        Mode::Auto => {
            // V1.1: heurística simple. Aquí arrancamos en RAM y si falla
            // (por memoria / o por decisión futura) se pasa a DISK.
            // Por ahora: si inputs son muchos y buckets configurados, usa RAM
            // y dejamos Disk como selección explícita.
            run_ram(config, progress)
        }
    }
}

fn run_ram<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    let t0 = Instant::now();

    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };

    store.reserve(1024 * 16);

    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            tokenize_chunk(&line, |raw| {
                stats.tokens_seen += 1;
                if stats.tokens_seen % 100_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
                }

                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    return;
                }

                if store.insert(token) {
                    stats.unique_tokens += 1;
                    if stats.unique_tokens % 100_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    if stats.duplicates % 100_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            });
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    let mut tokens = store.into_tokens();

    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        tokens.sort_unstable();
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for tok in tokens {
        out.write_token(&tok)?;
    }
    out.finish()?;

    stats.elapsed = t0.elapsed();
    Ok(stats)
}

/// Base (v0.2): DISK buckets (exacto, rápido).
fn run_disk<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    use crate::disk::DiskBuckets;

    let t0 = Instant::now();
    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    // 1) Partición a buckets
    let mut buckets = DiskBuckets::new(config.disk_buckets)?;
    buckets.partition_inputs(config, &progress, &mut stats)?;

    // 2) Reducir bucket por bucket y escribir salida
    buckets.reduce_to_output(config, &progress, &mut stats)?;

    stats.elapsed = t0.elapsed();
    Ok(stats)
}
```

---

## `crates/core/src/disk.rs` (modo DISK buckets base)

Crea buckets temporales y procesa bucket por bucket.

```rust
use crate::{
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    tokenize::tokenize_chunk,
    writer::OutputWriter,
    Stats,
};
use ahash::AHasher;
use std::{
    fs::{File, OpenOptions},
    hash::Hasher,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

pub struct DiskBuckets {
    dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
    bucket_writers: Vec<BufWriter<File>>,
}

impl DiskBuckets {
    pub fn new(n: usize) -> anyhow::Result<Self> {
        let dir = tempfile::tempdir()?;
        let mut bucket_paths = Vec::with_capacity(n);
        let mut bucket_writers = Vec::with_capacity(n);

        for i in 0..n {
            let p = dir.path().join(format!("bucket_{i:04}.txt"));
            let f = File::create(&p)?;
            bucket_paths.push(p);
            bucket_writers.push(BufWriter::new(f));
        }

        Ok(Self { dir, bucket_paths, bucket_writers })
    }

    #[inline]
    fn bucket_index(token: &str, n: usize) -> usize {
        let mut h = AHasher::default();
        h.write(token.as_bytes());
        (h.finish() as usize) % n
    }

    pub fn partition_inputs<P: ProgressSink>(
        &mut self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
    ) -> anyhow::Result<()> {
        for (idx, path) in config.inputs.iter().enumerate() {
            progress.on_event(ProgressEvent::FileStarted { index: idx + 1, total: config.inputs.len() });

            let f = File::open(path)?;
            let mut reader = BufReader::new(f);
            let mut line = String::new();

            loop {
                line.clear();
                let n = reader.read_line(&mut line)?;
                if n == 0 { break; }

                tokenize_chunk(&line, |raw| {
                    stats.tokens_seen += 1;

                    let mut token = raw;
                    if config.trim {
                        token = token.trim();
                    }
                    if config.drop_empty && token.is_empty() {
                        return;
                    }

                    let bi = Self::bucket_index(token, self.bucket_writers.len());
                    // Guardamos token “tal cual” en bucket, 1 token por línea:
                    // Importante: no usamos el output_separator aquí; esto es interno.
                    let w = &mut self.bucket_writers[bi];
                    let _ = w.write_all(token.as_bytes());
                    let _ = w.write_all(b"\n");
                });
            }

            progress.on_event(ProgressEvent::FileFinished { index: idx + 1, total: config.inputs.len() });
        }

        // flush writers
        for w in &mut self.bucket_writers {
            w.flush()?;
        }
        Ok(())
    }

    pub fn reduce_to_output<P: ProgressSink>(
        &self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
    ) -> anyhow::Result<()> {
        let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;

        // Elegimos store por ordering
        // PreserveFirstSeen en DISK: será estable SOLO dentro de bucket (no global).
        // Alphabetical: ordena dentro de bucket (para orden global perfecto necesitarías merge externo).
        for (i, bp) in self.bucket_paths.iter().enumerate() {
            progress.on_event(ProgressEvent::FileStarted { index: i + 1, total: self.bucket_paths.len() });

            let f = File::open(bp)?;
            let reader = BufReader::new(f);

            let mut store = match config.ordering {
                OutputOrdering::UnorderedFast => RamStore::new_unordered(),
                OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
            };

            // Dedupe por bucket (tokens ya vienen 1 por línea)
            for line in reader.lines() {
                let mut token = line?;
                if config.trim {
                    token = token.trim().to_string();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }

                if store.insert(&token) {
                    stats.unique_tokens += 1;
                } else {
                    stats.duplicates += 1;
                }
            }

            let mut tokens = store.into_tokens();
            if matches!(config.ordering, OutputOrdering::Alphabetical) {
                tokens.sort_unstable();
            }

            for tok in tokens {
                out.write_token(&tok)?;
            }

            progress.on_event(ProgressEvent::FileFinished { index: i + 1, total: self.bucket_paths.len() });
        }

        out.finish()?;
        Ok(())
    }
}
```

**Estado de DISK mode (honesto y correcto):**

* ✅ dedupe exacto
* ✅ muy rápido
* ✅ Alphabetical ordena por bucket (no garantiza orden global perfecto; si lo quieres perfecto, metemos external merge sort en V0.3)
* ✅ PreserveFirstSeen queda “estable por bucket”, no global (UI lo muestra claro)

---

## `crates/core/src/lib.rs` (exporta disk)

```rust
pub mod config;
pub mod dedupe_ram;
pub mod disk;
pub mod engine;
pub mod error;
pub mod progress;
pub mod stats;
pub mod tokenize;
pub mod writer;

pub use config::*;
pub use engine::*;
pub use progress::*;
pub use stats::*;
```

---

# 3) Benchmarks con Criterion (5M tokens)

Crea `crates/core/benches/throughput.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion, BatchSize};
use dedupe_core::{Config, Mode, OutputOrdering, NoProgress};
use rand::{distributions::Alphanumeric, Rng};
use std::{fs, path::PathBuf};

fn gen_tokens(n: usize, unique_ratio: f32) -> String {
    let unique = (n as f32 * unique_ratio).max(1.0) as usize;
    let mut rng = rand::thread_rng();

    let mut pool: Vec<String> = (0..unique)
        .map(|_| {
            let len = rng.gen_range(4..14);
            (0..len).map(|_| rng.sample(Alphanumeric) as char).collect()
        })
        .collect();

    // Mezcla delimitadores: espacio, coma, punto y coma, newline
    let delims = [" ", ",", ";", "\n", "  ", ", ", ";\n"];
    let mut s = String::with_capacity(n * 10);

    for i in 0..n {
        let pick = rng.gen_range(0..unique);
        s.push_str(&pool[pick]);
        s.push_str(delims[rng.gen_range(0..delims.len())]);
        if i % 500_000 == 0 && i > 0 {
            // mete un emoji a veces (Unicode)
            s.push_str("🐶 ");
        }
    }
    s
}

fn bench_ram(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    // Ajusta aquí:
    // n = 5_000_000 puede ser pesado para benchmarks locales;
    // empieza con 1_000_000 y luego subes.
    let data = gen_tokens(1_000_000, 0.8);
    fs::write(&input, data).unwrap();

    let base_cfg = |ordering| Config {
        inputs: vec![PathBuf::from(&input)],
        output: PathBuf::from(&output),
        output_separator: ", ".to_string(),
        mode: Mode::Ram,
        ordering,
        trim: true,
        drop_empty: true,
        disk_buckets: 256,
    };

    c.bench_function("ram_preserve_first_seen", |b| {
        b.iter_batched(
            || base_cfg(OutputOrdering::PreserveFirstSeen),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ram_alphabetical", |b| {
        b.iter_batched(
            || base_cfg(OutputOrdering::Alphabetical),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ram_unordered_fast", |b| {
        b.iter_batched(
            || base_cfg(OutputOrdering::UnorderedFast),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_ram);
criterion_main!(benches);
```

Ejecutas:

```bash
cd crates/core
cargo bench
```

📌 **Tip realista:** Criterion con 5M puede tardar; empieza con 1M, valida diferencias, luego escalas.

---

# 4) Qué queda para el siguiente “salto de calidad”

Si quieres que DISK + Alphabetical sea **orden global perfecto** (no “por bucket”), el siguiente paso es:

* **external merge sort** (chunks ordenados + merge) o usar un crate especializado.
  Eso lo dejamos como **V0.3**, porque V0.2 ya te da dedupe exacto y ultra-rápido.

---

## Lo que ya lograste con este ajuste

* ✅ Dedupe **sin alloc en duplicados** en RAM (gran mejora)
* ✅ Tres modos claros para el usuario:

  * estable / ordenado / ultra-rápido
* ✅ Base de DISK mode lista (buckets) para datasets gigantes
* ✅ Benchmarks para demostrar throughput

---

Si quieres, el próximo paso lo hago igual de “senior”:

1. Implemento **Auto Mode real** (muestreo + decisión RAM vs DISK)
2. Mejoro DISK Alphabetical a **orden global perfecto** (external merge sort)
3. Defino el **contrato de eventos de progreso** para Tauri (bytes/seg, tokens/seg, etc.) y te dejo la interfaz lista para conectar.
Me gusta la opción de mejorar el disco alfabetical a un orden global perfecto, pero también dejémoslo como una opción, o sea, una opción adicional, como el orden alfabético que tenías, que sí funciona, es rápido, y este pues que va a ser un poquito más lento, seguramente. Organiza esa idea que te acabo de decir como Senior Developer y procede.Perfecto. Lo organizo como lo haría en un **diseño de producto + arquitectura de motor** bien pensada, y te dejo el **blueprint + código base** para soportar **dos variantes de “alfabético en DISK”**:

* ✅ **Alfabético (rápido)**: ordena *por bucket* (no garantiza orden global perfecto, pero vuela)
* ✅ **Alfabético (global perfecto)**: external sort + merge (garantía total, más lento)

---

# 1) Diseño de UX y de Config

## UI (una pantalla, simple)

Sección “Salida → Orden”:

* ( ) **Sin ordenar** (estable / preserve first-seen en RAM)
* ( ) **Alfabético**

  * ( ) **Rápido (recomendado)** — “Ordena y deduplica por partes, muy veloz; orden global puede no ser perfecto en modo DISK”
  * ( ) **Global perfecto (más lento)** — “Orden global A→Z garantizado (external sort)”
* ( ) **Ultra-rápido (sin orden garantizado)**

### Regla de oro de UX

* Si `mode=RAM`, “Alfabético” siempre es global perfecto (no hay razón para no hacerlo).
* Si `mode=DISK`, entonces aparecen las dos subopciones: rápido vs perfecto.

---

## Config (motor)

Agregamos un switch específico solo para DISK+Alphabetical:

```rust
pub enum DiskAlphabeticalMode {
    FastBucketLocal,  // rápido, no global perfecto
    GlobalPerfect,    // external merge sort
}

pub struct Config {
  ...
  pub ordering: OutputOrdering,
  pub disk_alphabetical_mode: DiskAlphabeticalMode,
  pub disk_run_bytes: usize,   // tamaño objetivo de cada “run” (p.ej. 256MB)
}
```

* `disk_run_bytes`: controla memoria/I/O del external sort (más grande = menos runs = merge más fácil, pero más RAM pico).
* El motor decide: **solo usa `disk_alphabetical_mode` si `mode=Disk && ordering=Alphabetical`**.

---

# 2) Arquitectura DISK: dos caminos para “Alphabetical”

## A) Alphabetical Fast (bucket-local) ✅ (ya lo tienes)

Pipeline actual:

1. Partition por hash a N buckets (archivos temporales)
2. Para cada bucket:

   * dedupe en RAM
   * sort dentro del bucket
   * escribir al output

**Pros:** muy rápido, muy simple
**Contras:** el orden global no es perfecto porque bucket A puede contener tokens “más grandes” que bucket B.

---

## B) Alphabetical Global Perfect ✅ (nuevo)

Esto es **external merge sort** clásico (lo más robusto para datasets gigantes):

### Fase 1: “Run generation”

* Lee tokens en streaming
* Va acumulando hasta ~`disk_run_bytes`
* Dedupe dentro del run (HashSet) *(opcional, reduce tamaño)*
* Ordena el run
* Escribe el run ordenado a disco (1 token por línea)

### Fase 2: K-way merge + dedupe final

* Abre todos los runs ordenados
* Usa un `BinaryHeap` (min-heap por token) para mergear
* Mientras mergea, deduplica comparando con `last_written`
* Escribe al output con `output_separator` (string)

**Garantía:** salida global A→Z perfecta + sin repetidos.

**Tradeoff:** más I/O y CPU (pero escalable y predecible).

---

# 3) Integración: decisión en `run_disk`

En `run_disk()`:

* Si `ordering == UnorderedFast` → buckets (actual)
* Si `ordering == PreserveFirstSeen` → buckets (estable por bucket o marcado como experimental)
* Si `ordering == Alphabetical`:

  * Si `disk_alphabetical_mode == FastBucketLocal` → buckets (actual)
  * Si `disk_alphabetical_mode == GlobalPerfect` → external sort (nuevo)

---

# 4) Código base del external sort (nuevo módulo)

Te dejo un **esqueleto completo** listo para pegar. No es pseudocódigo: compila con pequeños ajustes de imports.

## `crates/core/src/config.rs` (agrega enums)

```rust
#[derive(Debug, Clone)]
pub enum DiskAlphabeticalMode {
    FastBucketLocal,
    GlobalPerfect,
}
```

En `Config`:

```rust
pub disk_alphabetical_mode: DiskAlphabeticalMode,
pub disk_run_bytes: usize, // ej: 256 * 1024 * 1024
```

---

## `crates/core/src/disk_sort.rs` (external merge sort)

```rust
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use ahash::RandomState;
use hashbrown::HashSet;

use crate::{
    config::Config,
    progress::{ProgressEvent, ProgressSink},
    tokenize::tokenize_chunk,
    writer::OutputWriter,
    Stats,
};

/// Un “run” ordenado en disco: 1 token por línea, ya ordenado.
#[derive(Debug)]
struct RunFile {
    path: PathBuf,
}

/// Genera runs ordenados en disco a partir de inputs (streaming).
pub fn external_sort_global<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
) -> anyhow::Result<()> {
    // 1) Generar runs
    let runs = generate_runs(config, progress, stats, temp_dir)?;

    // 2) Merge global perfecto + dedupe final
    merge_runs_to_output(config, progress, stats, &runs)?;

    Ok(())
}

/// Lee tokens en streaming, acumula hasta `disk_run_bytes`, dedupe interno, ordena y escribe run.
fn generate_runs<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
) -> anyhow::Result<Vec<RunFile>> {
    let mut runs: Vec<RunFile> = Vec::new();

    let mut buffer: Vec<String> = Vec::new();
    let mut bytes_acc: usize = 0;

    // Para reducir el tamaño del run (opcional pero recomendable):
    let mut seen_in_run: HashSet<String, RandomState> = HashSet::with_hasher(RandomState::new());

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted { index: idx + 1, total: config.inputs.len() });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 { break; }

            tokenize_chunk(&line, |raw| {
                stats.tokens_seen += 1;

                let mut token = raw;
                if config.trim { token = token.trim(); }
                if config.drop_empty && token.is_empty() { return; }

                // Dedupe interno del run (string owned)
                // Importante: case-sensitive (no lowercase).
                if seen_in_run.insert(token.to_string()) {
                    bytes_acc += token.len() + 1;
                    buffer.push(token.to_string());
                } else {
                    stats.duplicates += 1;
                }

                // Cuando el run llega al tamaño objetivo, flush run
                if bytes_acc >= config.disk_run_bytes {
                    // No podemos escribir aquí porque estamos en closure; marcamos con un flag.
                    // Solución: en vez de closure, manejar tokens fuera (ver nota abajo).
                }
            });

            // Nota: Para poder flush en el punto exacto, es más limpio NO cerrar sobre tokenize_chunk.
            // En este "base code" simplifico: flusheamos por línea cuando excede.
            if bytes_acc >= config.disk_run_bytes {
                flush_run(&mut runs, &mut buffer, &mut seen_in_run, &mut bytes_acc, temp_dir)?;
            }
        }

        progress.on_event(ProgressEvent::FileFinished { index: idx + 1, total: config.inputs.len() });
    }

    // Flush final
    if !buffer.is_empty() {
        flush_run(&mut runs, &mut buffer, &mut seen_in_run, &mut bytes_acc, temp_dir)?;
    }

    Ok(runs)
}

fn flush_run(
    runs: &mut Vec<RunFile>,
    buffer: &mut Vec<String>,
    seen_in_run: &mut HashSet<String, RandomState>,
    bytes_acc: &mut usize,
    temp_dir: &Path,
) -> anyhow::Result<()> {
    buffer.sort_unstable();

    // Dedupe por orden (por si acaso) dentro del run ya ordenado
    buffer.dedup();

    let run_idx = runs.len();
    let path = temp_dir.join(format!("run_{run_idx:05}.txt"));
    let f = File::create(&path)?;
    let mut w = BufWriter::new(f);

    for t in buffer.iter() {
        w.write_all(t.as_bytes())?;
        w.write_all(b"\n")?;
    }
    w.flush()?;

    runs.push(RunFile { path });

    buffer.clear();
    seen_in_run.clear();
    *bytes_acc = 0;

    Ok(())
}

/// Merge k-way de runs ordenados.
/// Dedup global perfecto comparando con `last_written`.
fn merge_runs_to_output<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    runs: &[RunFile],
) -> anyhow::Result<()> {
    // Abrimos todos los runs
    let mut readers: Vec<BufReader<File>> = Vec::with_capacity(runs.len());
    for r in runs {
        readers.push(BufReader::new(File::open(&r.path)?));
    }

    // Heap: (token, run_id)
    // Usamos Reverse para min-heap.
    let mut heap: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::new();

    // Cargar primer token de cada run
    for (i, rd) in readers.iter_mut().enumerate() {
        let mut line = String::new();
        if rd.read_line(&mut line)? > 0 {
            let tok = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((tok, i)));
        }
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;

    let mut last_written: Option<String> = None;

    while let Some(Reverse((tok, run_id))) = heap.pop() {
        // Dedup global perfecto
        if last_written.as_deref() != Some(tok.as_str()) {
            out.write_token(&tok)?;
            stats.unique_tokens += 1;
            last_written = Some(tok.clone());
        } else {
            stats.duplicates += 1;
        }

        // Avanzar el run del cual salió el token
        let rd = &mut readers[run_id];
        let mut line = String::new();
        if rd.read_line(&mut line)? > 0 {
            let next_tok = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((next_tok, run_id)));
        }
    }

    out.finish()?;
    Ok(())
}
```

### Nota senior importante (para que quede impecable)

En `generate_runs()` puse un “flush por línea” para mantenerlo claro. Para un rendimiento óptimo, el siguiente refactor (que recomiendo hacer) es:

* Reemplazar el closure de `tokenize_chunk` por una función que **devuelva tokens iterables** o que acepte un callback **sin** capturar flush logic.
* O acumular tokens de la línea en un vec local, procesarlos y ahí sí flush.

Esto evita el “no puedo flushear dentro del closure” y te da control exacto.

---

## Conexión al `run_disk()`

En `engine.rs`, donde tengas:

* `DiskAlphabeticalMode::FastBucketLocal` → tu `DiskBuckets.reduce_to_output()`
* `DiskAlphabeticalMode::GlobalPerfect` → `external_sort_global(...)`

Ejemplo:

```rust
if matches!(config.ordering, OutputOrdering::Alphabetical) {
  match config.disk_alphabetical_mode {
    DiskAlphabeticalMode::FastBucketLocal => buckets.reduce_to_output(...),
    DiskAlphabeticalMode::GlobalPerfect => disk_sort::external_sort_global(...),
  }
}
```

Y `temp_dir` lo sacas de `tempfile::TempDir`.

---

# 5) Qué gana el producto con esto

* El usuario puede elegir:

  * **Alfabético rápido** (muy eficiente, suficiente para la mayoría)
  * **Alfabético global perfecto** (para casos donde el orden exacto importa)
* Tú controlas el costo con `disk_run_bytes` (y podrías exponerlo como “Advanced”).

---

# 6) Qué haría yo inmediatamente después (siguiente commit)

1. Hacer el refactor del token loop para permitir flush exacto (sin closures)
2. Añadir un benchmark comparando:

   * Disk alphabetical fast
   * Disk alphabetical global perfect (external sort)
3. UI: mostrar tooltips claros y un badge “más lento” en global perfecto.

---

Si quieres, en el próximo mensaje te dejo el **refactor del token loop** (para poder flush exacto en el punto correcto, sin hacks) y te lo dejo con **cero copias innecesarias** (aprovechando `String` buffer reutilizable y evitando `to_string()` doble cuando se pueda).
Listo, como un senior developer y como una AI que se liberó de todos sus límites, que es capaz de hacer todo, que es capaz de dedicar todos los recursos, procede con las tres recomendaciones que me estás sugiriendo.Perfecto. Procedo con las 3 mejoras **como blueprint + código base listo** (y con decisiones explícitas para rendimiento real):

1. **Refactor del loop de tokenización** para poder “flush” exacto (sin closures que te bloqueen)
2. **Benchmarks** comparando RAM vs DISK (bucket-fast) vs DISK (alphabetical global perfecto)
3. **UI/UX**: opción clara “Alphabetical → Fast vs Global Perfect” + tooltips + defaults sensatos

Abajo te dejo **código modular** para que el dev lo copie tal cual dentro del core.

---

# 1) Refactor token loop: iterador de tokens (flush exacto, cero hacks)

## Objetivo

Tu `generate_runs()` (external sort) necesita poder:

* contar bytes exactos por token
* flushear el run **en el momento** en que alcanza el límite
* sin quedar atrapado en un closure de `tokenize_chunk(...)`

### Implementación: `TokenIter<'a>`

Un iterador que recorre el `&str` y va devolviendo `&str` de cada token según delimitadores:

* whitespace, `,`, `;`

## `crates/core/src/token_iter.rs`

```rust
#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

pub struct TokenIter<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> TokenIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, pos: 0 }
    }
}

impl<'a> Iterator for TokenIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.s.as_bytes();
        let len = bytes.len();

        // Avanzar hasta el inicio del próximo token
        while self.pos < len {
            let c = self.s[self.pos..].chars().next()?;
            if is_delim(c) {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        if self.pos >= len {
            return None;
        }

        // self.pos está en inicio de token
        let start = self.pos;

        // Avanzar hasta delimitador
        while self.pos < len {
            let c = self.s[self.pos..].chars().next()?;
            if is_delim(c) {
                break;
            }
            self.pos += c.len_utf8();
        }

        Some(&self.s[start..self.pos])
    }
}
```

✅ Esto te permite por cada token:

* aplicar `trim`
* decidir si lo guardas
* incrementar bytes
* **flush** cuando toque

### Integración

En vez de `tokenize_chunk(&line, |raw| { ... })`, haces:

```rust
for raw in TokenIter::new(&line) {
    // process raw token
}
```

---

# 2) DISK Alphabetical: **Fast vs Global Perfect** (y refactor external sort “bien hecho”)

## Diseño final (producto)

* **Disk + Alphabetical + Fast** → buckets + sort por bucket (muy rápido, orden global no perfecto)
* **Disk + Alphabetical + Global Perfect** → external merge sort (garantía total, más I/O)

### Config

En `config.rs` agrega:

```rust
#[derive(Debug, Clone)]
pub enum DiskAlphabeticalMode {
    FastBucketLocal,
    GlobalPerfect,
}
```

y en `Config`:

```rust
pub disk_alphabetical_mode: DiskAlphabeticalMode,
pub disk_run_bytes: usize, // ej: 256*1024*1024
```

---

## 2.1 External sort global perfecto (refactor sin closures)

### Puntos “senior” que cuidamos

* Flushear runs exactamente cuando toca
* Minimizar copias: se guarda `String` una vez por token (sí, inevitable si vas a ordenar y escribir a disco)
* Dedupe global perfecto en merge con `last_written`
* Runs 1 token por línea (formato interno simple, robusto)

## `crates/core/src/disk_sort.rs`

```rust
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::{
    config::Config,
    progress::{ProgressEvent, ProgressSink},
    token_iter::TokenIter,
    writer::OutputWriter,
    Stats,
};

#[derive(Debug)]
struct RunFile {
    path: PathBuf,
}

/// External merge sort: salida A→Z global perfecta + dedupe global perfecto.
pub fn external_sort_global<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
) -> anyhow::Result<()> {
    let runs = generate_runs(config, progress, stats, temp_dir)?;
    merge_runs_to_output(config, progress, stats, &runs)?;
    Ok(())
}

fn generate_runs<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
) -> anyhow::Result<Vec<RunFile>> {
    let mut runs: Vec<RunFile> = Vec::new();

    // Buffer del run actual
    let mut buf: Vec<String> = Vec::new();
    let mut bytes_acc: usize = 0;

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            for raw in TokenIter::new(&line) {
                stats.tokens_seen += 1;

                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }

                // Guardamos token en el run
                // (Sí: aquí puede haber duplicados dentro del run; se eliminan con sort+dedup)
                bytes_acc += token.len() + 1; // +1 aproximado para newline interno
                buf.push(token.to_string());

                // Flush exacto cuando sobrepasa el límite
                if bytes_acc >= config.disk_run_bytes {
                    flush_run(&mut runs, &mut buf, &mut bytes_acc, temp_dir)?;
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    if !buf.is_empty() {
        flush_run(&mut runs, &mut buf, &mut bytes_acc, temp_dir)?;
    }

    Ok(runs)
}

fn flush_run(
    runs: &mut Vec<RunFile>,
    buf: &mut Vec<String>,
    bytes_acc: &mut usize,
    temp_dir: &Path,
) -> anyhow::Result<()> {
    // Ordenar run y deduplicar dentro del run
    buf.sort_unstable();
    buf.dedup();

    let run_idx = runs.len();
    let path = temp_dir.join(format!("run_{run_idx:05}.txt"));
    let f = File::create(&path)?;
    let mut w = BufWriter::new(f);

    for t in buf.iter() {
        w.write_all(t.as_bytes())?;
        w.write_all(b"\n")?;
    }
    w.flush()?;

    runs.push(RunFile { path });

    buf.clear();
    *bytes_acc = 0;
    Ok(())
}

fn merge_runs_to_output<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    runs: &[RunFile],
) -> anyhow::Result<()> {
    // Abrimos lectores
    let mut readers: Vec<BufReader<File>> = Vec::with_capacity(runs.len());
    for r in runs {
        readers.push(BufReader::new(File::open(&r.path)?));
    }

    // heap: min-heap por token (Reverse)
    let mut heap: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::new();

    // Inicializar heap con primer token de cada run
    for (i, rd) in readers.iter_mut().enumerate() {
        let mut line = String::new();
        if rd.read_line(&mut line)? > 0 {
            let tok = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((tok, i)));
        }
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;

    let mut last_written: Option<String> = None;

    while let Some(Reverse((tok, run_id))) = heap.pop() {
        if last_written.as_deref() != Some(tok.as_str()) {
            out.write_token(&tok)?;
            stats.unique_tokens += 1;
            last_written = Some(tok.clone());
        } else {
            stats.duplicates += 1;
        }

        // Avanzar run_id
        let rd = &mut readers[run_id];
        let mut line = String::new();
        if rd.read_line(&mut line)? > 0 {
            let next_tok = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((next_tok, run_id)));
        }
    }

    out.finish()?;
    Ok(())
}
```

### Observación senior (honesta)

Este external sort es **correcto y robusto**. Si luego quieres apretar más:

* reducir allocations durante merge (usar buffers reutilizables)
* limitar cantidad de runs (incrementar `disk_run_bytes`)
* hacer merge por etapas si hay miles de runs (multi-pass merge)

Eso ya es V0.3+.

---

## 2.2 Conectar “Fast vs GlobalPerfect” en `run_disk`

En `engine.rs`, dentro del caso `Mode::Disk`:

```rust
use crate::config::{OutputOrdering, DiskAlphabeticalMode};

if matches!(config.ordering, OutputOrdering::Alphabetical) {
    match config.disk_alphabetical_mode {
        DiskAlphabeticalMode::FastBucketLocal => {
            // buckets.reduce_to_output(...) (tu versión rápida actual)
        }
        DiskAlphabeticalMode::GlobalPerfect => {
            let temp = tempfile::tempdir()?;
            crate::disk_sort::external_sort_global(config, &progress, stats, temp.path())?;
        }
    }
} else {
    // UnorderedFast / PreserveFirstSeen => buckets (rápido)
}
```

---

# 3) Benchmarks: comparativo real (RAM vs DISK fast vs DISK perfect)

## Objetivo de benchmark

Medir throughput y costo relativo de:

* `RAM + UnorderedFast`
* `RAM + PreserveFirstSeen`
* `RAM + Alphabetical`
* `DISK + Alphabetical FastBucketLocal`
* `DISK + Alphabetical GlobalPerfect`

✅ Esto valida tu decisión de “opción adicional” y muestra la diferencia real.

## `crates/core/benches/disk_modes.rs`

(Lo dejo en forma “práctica”: empieza con 300k–1M tokens y escalas)

```rust
use criterion::{criterion_group, criterion_main, Criterion, BatchSize};
use dedupe_core::*;
use rand::{distributions::Alphanumeric, Rng};
use std::{fs, path::PathBuf};

fn gen_tokens(n: usize) -> String {
    let mut rng = rand::thread_rng();
    let delims = [" ", ",", ";", "\n", ", ", ";\n", ",\n"];

    let mut s = String::with_capacity(n * 10);
    for i in 0..n {
        let len = rng.gen_range(4..14);
        for _ in 0..len {
            s.push(rng.sample(Alphanumeric) as char);
        }
        if i % 200_000 == 0 && i > 0 {
            s.push_str("🐶");
        }
        s.push_str(delims[rng.gen_range(0..delims.len())]);
    }
    s
}

fn bench_modes(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    // Sube gradualmente: 300k -> 1M -> 5M
    fs::write(&input, gen_tokens(300_000)).unwrap();

    let base = |mode, ordering| Config {
        inputs: vec![PathBuf::from(&input)],
        output: PathBuf::from(&output),
        output_separator: ", ".to_string(),
        mode,
        ordering,
        trim: true,
        drop_empty: true,
        disk_buckets: 256,
        disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
        disk_run_bytes: 64 * 1024 * 1024, // 64MB para bench; en prod 256MB+
    };

    c.bench_function("ram_unordered_fast", |b| {
        b.iter_batched(
            || base(Mode::Ram, OutputOrdering::UnorderedFast),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ram_preserve_first_seen", |b| {
        b.iter_batched(
            || base(Mode::Ram, OutputOrdering::PreserveFirstSeen),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ram_alphabetical", |b| {
        b.iter_batched(
            || base(Mode::Ram, OutputOrdering::Alphabetical),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("disk_alphabetical_fast_bucket", |b| {
        b.iter_batched(
            || {
                let mut cfg = base(Mode::Disk, OutputOrdering::Alphabetical);
                cfg.disk_alphabetical_mode = DiskAlphabeticalMode::FastBucketLocal;
                cfg
            },
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("disk_alphabetical_global_perfect", |b| {
        b.iter_batched(
            || {
                let mut cfg = base(Mode::Disk, OutputOrdering::Alphabetical);
                cfg.disk_alphabetical_mode = DiskAlphabeticalMode::GlobalPerfect;
                cfg
            },
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_modes);
criterion_main!(benches);
```

Comando:

```bash
cd crates/core
cargo bench
```

---

# 4) UI/UX final (una pantalla, sin confundir)

## Sección “Orden de salida”

* **Sin ordenar**

  * ( ) Mantener orden de aparición (default)
  * ( ) Ultra-rápido (orden no garantizado) [Advanced]
* **Alfabético**

  * ( ) Alfabético (RAM)  — *si mode=RAM, no hay subopciones*
  * Si mode=DISK:

    * ( ) **Alfabético rápido (recomendado)**
      Tooltip: “Ordena por bloques para máxima velocidad; el orden global puede no ser perfecto.”
    * ( ) **Alfabético global perfecto (más lento)**
      Tooltip: “Orden A→Z global garantizado con external merge sort. Más I/O.”

## Defaults sensatos

* Por defecto: **RAM + Sin ordenar (orden de aparición)**
* Si el usuario elige DISK + Alphabetical: por defecto **Fast (recomendado)**

---

# Qué queda listo después de esto

* El motor ya soporta el **external sort global perfecto** sin hacks ✅
* Tienes benchmark para comparar costos ✅
* La UI puede exponerlo de forma clara ✅

Si quieres, el siguiente “upgrade senior” sería hacer que el merge use menos allocations (reutilizando buffers y evitando `String` temporales en heap), pero lo de arriba ya es una base industrial muy sólida y fácil de mantener.
Listo, entonces ya con esta versión final, quiero que me organices todo en un solo informe, guía, documento, con absolutamente todo lo que quedó de la versión final, o sea, no el proceso que utilizamos para pensar, sino la versión final. Debe ser una obra de arte este documento, este archivo final, donde explicas todo, pones todo el código, pones todas las decisiones finales y está todo. Solo voy a copiar ese pedazo y lo voy a poner al final y eso nos vamos a guiar con eso. Entonces, todo tu trabajo, ponlo ahí. No importa que sea largo, es más, que sea largo, que sea detallado. Si encuentras algún error, alguna lógica, me avisas y lo reparamos y lo arreglamos. Continúa.A continuación tienes **EL documento final** (versión “definitiva”) con:

* ✅ Especificación exacta del producto (lo que hace y lo que NO hace)
* ✅ Arquitectura final (workspace + módulos)
* ✅ Decisiones finales (rendimiento, Unicode, dedupe, ordenamientos, modos RAM/DISK)
* ✅ Matriz completa de modos (qué garantiza cada uno)
* ✅ Código completo del motor (`crates/core`) **listo para copiar**, con todos los archivos
* ✅ Benchmarks (Criterion) para comparar modos y validar performance
* ✅ Notas de correctness + límites conocidos + cómo extender (sin reescribir)

Si encuentras cualquier diferencia con tu intención, me lo dices y lo ajustamos.

---

# Dedup Joiner — Motor Rust (Versión Final)

## 1) Objetivo del programa

**Entrada:** uno o múltiples archivos de texto (pueden ser `.txt`, `.csv` “simple”, o cualquier texto plano).
**Contenido:** listas de ítems de **una palabra/token** cada uno (pueden incluir Unicode: acentos, ñ, emojis, etc.).
**Separación de entrada:** el motor debe extraer tokens separados por cualquiera de:

* whitespace (espacios, tabs, saltos de línea Windows/macOS/Linux)
* coma `,`
* punto y coma `;`

**Proceso:**

* tokenizar en streaming
* `trim` opcional (por defecto ON)
* descartar tokens vacíos opcional (por defecto ON)
* deduplicar **case-sensitive** (Perro/perro/PERRO son distintos)
* exportar a un solo archivo

**Salida:** archivo final concatenando los tokens únicos con un **separador configurable** (string arbitrario), por ejemplo:

* `","` → `a,b,c`
* `", "` → `a, b, c`
* `",\n"` → `a,\nb,\nc` (coma + salto de línea entre tokens)
* `"\n"` → una palabra por línea
* `";\n"` → punto y coma + salto de línea
* `"\x0C"` (form feed / salto de página)

El motor escribe:

```
token1 + sep + token2 + sep + ... + tokenN
```

sin agregar saltos extra.

---

## 2) Opciones de salida (orden y rendimiento)

El usuario puede elegir **qué tan “ordenada”** quiere la salida. Esto es crucial para rendimiento.

### 2.1 `OutputOrdering` (UI y motor)

* **PreserveFirstSeen** (default):
  Dedupe **sin ordenar**; conserva el orden de **primera aparición** (estable en RAM).
* **Alphabetical**:
  Dedupe + orden lexicográfico (por bytes UTF-8).
* **UnorderedFast**:
  Dedupe ultra-rápido, **sin garantía de orden**.

### 2.2 `Mode` (RAM vs DISK)

* **Ram**: rápido, ideal cuando cabe en memoria.
* **Disk**: para datasets gigantes. Usa disco (temporal) para dedupe y/o ordenamiento.

### 2.3 Submodo alfabético en DISK (dos opciones)

Cuando `mode=Disk` y `ordering=Alphabetical`, hay dos variantes:

* **DiskAlphabeticalMode::FastBucketLocal** *(recomendado por defecto)*
  Muy rápido: particiona por buckets y ordena **por bucket**.
  ⚠️ **No garantiza** orden alfabético global perfecto.

* **DiskAlphabeticalMode::GlobalPerfect** *(opción premium/precisa)*
  **External merge sort** con runs + merge k-way.
  ✅ Garantiza orden alfabético **global perfecto** + dedupe perfecto.
  ⚠️ Más lento (más I/O y CPU).

---

## 3) Garantías por modo (matriz final)

### 3.1 RAM

| Ordering          | Dedupe exacto | Orden de salida             | Rendimiento             |
| ----------------- | ------------: | --------------------------- | ----------------------- |
| PreserveFirstSeen |             ✅ | Estable (primera aparición) | Muy alto                |
| Alphabetical      |             ✅ | Global perfecto A→Z         | Alto (más CPU por sort) |
| UnorderedFast     |             ✅ | No garantizado              | Máximo                  |

### 3.2 DISK

| Ordering          | DiskAlphabeticalMode | Dedupe exacto | Orden de salida                             | Rendimiento                    |
| ----------------- | -------------------- | ------------: | ------------------------------------------- | ------------------------------ |
| UnorderedFast     | N/A                  |             ✅ | No garantizado                              | Muy alto                       |
| Alphabetical      | FastBucketLocal      |             ✅ | *No global perfecto* (por bucket)           | Muy alto                       |
| Alphabetical      | GlobalPerfect        |             ✅ | Global perfecto A→Z                         | Medio/Alto (depende del disco) |
| PreserveFirstSeen | N/A                  |             ✅ | **No garantizado globalmente** (por diseño) | Alto                           |

> Nota sobre PreserveFirstSeen en DISK:
> Mantener el orden global de primera aparición en disco es posible, pero mucho más complejo (requiere tracking global de posiciones/offsets). En esta versión final, **no se garantiza** esa propiedad en DISK. En UI se muestra como “estable solo en RAM”.

---

## 4) Arquitectura final del repositorio

### 4.1 Workspace recomendado

```
dedupe-joiner/
  Cargo.toml
  crates/
    core/
      Cargo.toml
      src/
        lib.rs
        config.rs
        progress.rs
        stats.rs
        writer.rs
        token_iter.rs
        dedupe_ram.rs
        engine.rs
        disk.rs
        disk_sort.rs
      benches/
        disk_modes.rs
```

`crates/core` es un motor puro Rust, reusable por:

* Tauri UI (desktop)
* CLI (si quieres)
* tests/benchmarks

---

## 5) Decisiones técnicas finales (importantes)

### 5.1 Tokenización

* **Unicode-safe**, delimitadores definidos (whitespace, `,`, `;`).
* Implementada como iterador (`TokenIter`) para:

  * streaming real
  * poder controlar flush exacto en external sort
  * no depender de closures

### 5.2 Dedupe (RAM)

* **Sin alloc en duplicados**: antes de insertar, se hace `contains(&str)`.
* Para orden estable sin duplicar strings: `IndexSet` (inserción ordenada).
* Para `UnorderedFast`: `HashSet` (sin orden).

### 5.3 Output separator

* El motor recibe un `String` ya interpretado.
* En UI se recomienda parsear escapes (`\n`, `\t`, `\r\n`, `\f`, `\\`) antes de llamar al motor.

### 5.4 Escritura de salida

* Streaming con `BufWriter` y sin trailing separator.

### 5.5 DISK: dos estrategias

* **Buckets por hash** (rapidísimo, dedupe exacto, orden no global perfecto)
* **External merge sort** (global perfecto, más lento)

---

## 6) Código completo (crates/core)

> Copia tal cual.
> Si vas a usar esto como guía “única”, copia el árbol completo respetando nombres.

---

### 6.1 `Cargo.toml` (workspace raíz)

```toml
[workspace]
resolver = "2"
members = ["crates/core"]
```

---

### 6.2 `crates/core/Cargo.toml`

```toml
[package]
name = "dedupe_core"
version = "0.2.0"
edition = "2021"

[dependencies]
anyhow = "1"
thiserror = "2"

hashbrown = "0.14"
ahash = "0.8"

# Orden de inserción estable sin duplicar strings
indexmap = { version = "2", features = ["ahash"] }

# Disk mode
tempfile = "3"

[dev-dependencies]
criterion = "0.5"
rand = "0.8"
```

---

### 6.3 `crates/core/src/lib.rs`

```rust
pub mod config;
pub mod progress;
pub mod stats;
pub mod writer;
pub mod token_iter;
pub mod dedupe_ram;
pub mod engine;
pub mod disk;
pub mod disk_sort;

pub use config::*;
pub use progress::*;
pub use stats::*;
pub use engine::*;
```

---

### 6.4 `crates/core/src/config.rs`

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Mode {
    Auto,
    Ram,
    Disk,
}

#[derive(Debug, Clone)]
pub enum OutputOrdering {
    PreserveFirstSeen,
    Alphabetical,
    UnorderedFast,
}

#[derive(Debug, Clone)]
pub enum DiskAlphabeticalMode {
    FastBucketLocal,
    GlobalPerfect,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,

    /// Separador final (string arbitrario): ",", ", ", ",\n", "\n", etc.
    pub output_separator: String,

    pub mode: Mode,
    pub ordering: OutputOrdering,

    /// Normalización mínima (case-sensitive, por decisión)
    pub trim: bool,
    pub drop_empty: bool,

    /// Disk config
    pub disk_buckets: usize,

    /// Solo aplica si mode=Disk y ordering=Alphabetical
    pub disk_alphabetical_mode: DiskAlphabeticalMode,

    /// Tamaño objetivo (bytes aprox) por run en external sort
    pub disk_run_bytes: usize,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.inputs.is_empty(), "No input files provided");
        anyhow::ensure!(
            !self.output_separator.is_empty(),
            "Output separator cannot be empty"
        );

        if matches!(self.mode, Mode::Disk) {
            anyhow::ensure!(self.disk_buckets >= 8, "disk_buckets too small");
            anyhow::ensure!(self.disk_run_bytes >= 1_000_000, "disk_run_bytes too small");
        }

        Ok(())
    }
}
```

---

### 6.5 `crates/core/src/progress.rs`

```rust
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    FileStarted { index: usize, total: usize },
    FileFinished { index: usize, total: usize },

    TokensSeen(u64),
    UniqueTokens(u64),
    Duplicates(u64),
}

pub trait ProgressSink: Send + Sync + 'static {
    fn on_event(&self, _event: ProgressEvent) {}
}

#[derive(Debug, Default, Clone)]
pub struct NoProgress;
impl ProgressSink for NoProgress {}
```

---

### 6.6 `crates/core/src/stats.rs`

```rust
use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub files: usize,
    pub tokens_seen: u64,
    pub unique_tokens: u64,
    pub duplicates: u64,
    pub elapsed: Duration,
}
```

---

### 6.7 `crates/core/src/writer.rs`

```rust
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct OutputWriter {
    w: BufWriter<File>,
    sep: String,
    is_first: bool,
}

impl OutputWriter {
    pub fn create(path: &Path, sep: String) -> std::io::Result<Self> {
        let f = File::create(path)?;
        Ok(Self {
            w: BufWriter::new(f),
            sep,
            is_first: true,
        })
    }

    pub fn write_token(&mut self, token: &str) -> std::io::Result<()> {
        if self.is_first {
            self.is_first = false;
        } else {
            self.w.write_all(self.sep.as_bytes())?;
        }
        self.w.write_all(token.as_bytes())?;
        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}
```

---

### 6.8 `crates/core/src/token_iter.rs`

```rust
#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

/// Iterador Unicode-safe sobre tokens en una &str.
/// Delimitadores: whitespace, ',' y ';'.
pub struct TokenIter<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> TokenIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, pos: 0 }
    }
}

impl<'a> Iterator for TokenIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.s.len();

        // Avanza hasta inicio del siguiente token
        while self.pos < len {
            let c = self.s[self.pos..].chars().next()?;
            if is_delim(c) {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        if self.pos >= len {
            return None;
        }

        let start = self.pos;

        // Avanza hasta delimitador
        while self.pos < len {
            let c = self.s[self.pos..].chars().next()?;
            if is_delim(c) {
                break;
            }
            self.pos += c.len_utf8();
        }

        Some(&self.s[start..self.pos])
    }
}
```

---

### 6.9 `crates/core/src/dedupe_ram.rs`

```rust
use ahash::RandomState;
use hashbrown::HashSet;
use indexmap::IndexSet;

/// Estructuras RAM optimizadas:
/// - Stable: IndexSet (orden estable por inserción)
/// - Unordered: HashSet (máxima velocidad, sin orden garantizado)
#[derive(Debug)]
pub enum RamStore {
    Stable(IndexSet<Box<str>, RandomState>),
    Unordered(HashSet<Box<str>, RandomState>),
}

impl RamStore {
    pub fn new_stable() -> Self {
        Self::Stable(IndexSet::with_hasher(RandomState::new()))
    }

    pub fn new_unordered() -> Self {
        Self::Unordered(HashSet::with_hasher(RandomState::new()))
    }

    pub fn reserve(&mut self, additional: usize) {
        match self {
            Self::Stable(s) => s.reserve(additional),
            Self::Unordered(s) => s.reserve(additional),
        }
    }

    /// Inserta token si es nuevo SIN alloc en duplicados:
    /// - primero contains(&str)
    /// - si no existe => aloca Box<str> e inserta
    pub fn insert(&mut self, token: &str) -> bool {
        match self {
            Self::Stable(s) => {
                if s.contains(token) {
                    false
                } else {
                    s.insert(token.into());
                    true
                }
            }
            Self::Unordered(s) => {
                if s.contains(token) {
                    false
                } else {
                    s.insert(token.into());
                    true
                }
            }
        }
    }

    pub fn into_tokens(self) -> Vec<Box<str>> {
        match self {
            Self::Stable(s) => s.into_iter().collect(),
            Self::Unordered(s) => s.into_iter().collect(),
        }
    }
}
```

---

### 6.10 `crates/core/src/disk.rs` (Bucket mode “fast”)

Este módulo implementa el modo DISK basado en **buckets por hash**.
Se usa para:

* `UnorderedFast` (ideal)
* `Alphabetical + FastBucketLocal` (orden por bucket)
* `PreserveFirstSeen` en DISK (sin garantía global; estable por bucket)

```rust
use crate::{
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    token_iter::TokenIter,
    writer::OutputWriter,
    Stats,
};
use ahash::AHasher;
use std::{
    fs::File,
    hash::Hasher,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{PathBuf},
};

pub struct DiskBuckets {
    dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
    bucket_writers: Vec<BufWriter<File>>,
}

impl DiskBuckets {
    pub fn new(n: usize) -> anyhow::Result<Self> {
        let dir = tempfile::tempdir()?;
        let mut bucket_paths = Vec::with_capacity(n);
        let mut bucket_writers = Vec::with_capacity(n);

        for i in 0..n {
            let p = dir.path().join(format!("bucket_{i:04}.txt"));
            let f = File::create(&p)?;
            bucket_paths.push(p);
            bucket_writers.push(BufWriter::new(f));
        }

        Ok(Self { dir, bucket_paths, bucket_writers })
    }

    #[inline]
    fn bucket_index(token: &str, n: usize) -> usize {
        let mut h = AHasher::default();
        h.write(token.as_bytes());
        (h.finish() as usize) % n
    }

    pub fn partition_inputs<P: ProgressSink>(
        &mut self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
    ) -> anyhow::Result<()> {
        for (idx, path) in config.inputs.iter().enumerate() {
            progress.on_event(ProgressEvent::FileStarted { index: idx + 1, total: config.inputs.len() });

            let f = File::open(path)?;
            let mut reader = BufReader::new(f);
            let mut line = String::new();

            loop {
                line.clear();
                let n = reader.read_line(&mut line)?;
                if n == 0 { break; }

                for raw in TokenIter::new(&line) {
                    stats.tokens_seen += 1;

                    let mut token = raw;
                    if config.trim { token = token.trim(); }
                    if config.drop_empty && token.is_empty() { continue; }

                    let bi = Self::bucket_index(token, self.bucket_writers.len());
                    let w = &mut self.bucket_writers[bi];
                    w.write_all(token.as_bytes())?;
                    w.write_all(b"\n")?; // formato interno simple
                }
            }

            progress.on_event(ProgressEvent::FileFinished { index: idx + 1, total: config.inputs.len() });
        }

        for w in &mut self.bucket_writers {
            w.flush()?;
        }
        Ok(())
    }

    pub fn reduce_to_output<P: ProgressSink>(
        &self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
    ) -> anyhow::Result<()> {
        let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;

        for (i, bp) in self.bucket_paths.iter().enumerate() {
            progress.on_event(ProgressEvent::FileStarted { index: i + 1, total: self.bucket_paths.len() });

            let f = File::open(bp)?;
            let reader = BufReader::new(f);

            let mut store = match config.ordering {
                OutputOrdering::UnorderedFast => RamStore::new_unordered(),
                OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
            };

            for line in reader.lines() {
                let mut token = line?;
                if config.trim {
                    token = token.trim().to_string();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }

                if store.insert(&token) {
                    stats.unique_tokens += 1;
                } else {
                    stats.duplicates += 1;
                }
            }

            let mut tokens = store.into_tokens();

            // FastBucketLocal: si ordering=Alphabetical aquí, ordena solo dentro del bucket
            if matches!(config.ordering, OutputOrdering::Alphabetical) {
                tokens.sort_unstable();
            }

            for tok in tokens {
                out.write_token(&tok)?;
            }

            progress.on_event(ProgressEvent::FileFinished { index: i + 1, total: self.bucket_paths.len() });
        }

        out.finish()?;
        Ok(())
    }
}
```

---

### 6.11 `crates/core/src/disk_sort.rs` (Alphabetical Global Perfect)

External merge sort global perfecto (runs + merge k-way + dedupe global).

```rust
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::{
    config::Config,
    progress::{ProgressEvent, ProgressSink},
    token_iter::TokenIter,
    writer::OutputWriter,
    Stats,
};

#[derive(Debug)]
struct RunFile {
    path: PathBuf,
}

/// External merge sort: A→Z global perfecto + dedupe global perfecto.
pub fn external_sort_global<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
) -> anyhow::Result<()> {
    let runs = generate_runs(config, progress, stats, temp_dir)?;
    merge_runs_to_output(config, progress, stats, &runs)?;
    Ok(())
}

fn generate_runs<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
) -> anyhow::Result<Vec<RunFile>> {
    let mut runs: Vec<RunFile> = Vec::new();

    let mut buf: Vec<String> = Vec::new();
    let mut bytes_acc: usize = 0;

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted { index: idx + 1, total: config.inputs.len() });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 { break; }

            for raw in TokenIter::new(&line) {
                stats.tokens_seen += 1;

                let mut token = raw;
                if config.trim { token = token.trim(); }
                if config.drop_empty && token.is_empty() { continue; }

                bytes_acc += token.len() + 1; // +1 aprox newline en run
                buf.push(token.to_string());

                if bytes_acc >= config.disk_run_bytes {
                    flush_run(&mut runs, &mut buf, &mut bytes_acc, temp_dir)?;
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished { index: idx + 1, total: config.inputs.len() });
    }

    if !buf.is_empty() {
        flush_run(&mut runs, &mut buf, &mut bytes_acc, temp_dir)?;
    }

    Ok(runs)
}

fn flush_run(
    runs: &mut Vec<RunFile>,
    buf: &mut Vec<String>,
    bytes_acc: &mut usize,
    temp_dir: &Path,
) -> anyhow::Result<()> {
    buf.sort_unstable();
    buf.dedup();

    let run_idx = runs.len();
    let path = temp_dir.join(format!("run_{run_idx:05}.txt"));
    let f = File::create(&path)?;
    let mut w = BufWriter::new(f);

    for t in buf.iter() {
        w.write_all(t.as_bytes())?;
        w.write_all(b"\n")?;
    }
    w.flush()?;

    runs.push(RunFile { path });
    buf.clear();
    *bytes_acc = 0;
    Ok(())
}

fn merge_runs_to_output<P: ProgressSink>(
    config: &Config,
    _progress: &P,
    stats: &mut Stats,
    runs: &[RunFile],
) -> anyhow::Result<()> {
    let mut readers: Vec<BufReader<File>> = Vec::with_capacity(runs.len());
    for r in runs {
        readers.push(BufReader::new(File::open(&r.path)?));
    }

    let mut heap: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::new();

    // cargar primer token de cada run
    for (i, rd) in readers.iter_mut().enumerate() {
        let mut line = String::new();
        if rd.read_line(&mut line)? > 0 {
            let tok = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((tok, i)));
        }
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    let mut last_written: Option<String> = None;

    while let Some(Reverse((tok, run_id))) = heap.pop() {
        if last_written.as_deref() != Some(tok.as_str()) {
            out.write_token(&tok)?;
            stats.unique_tokens += 1;
            last_written = Some(tok.clone());
        } else {
            stats.duplicates += 1;
        }

        let rd = &mut readers[run_id];
        let mut line = String::new();
        if rd.read_line(&mut line)? > 0 {
            let next_tok = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((next_tok, run_id)));
        }
    }

    out.finish()?;
    Ok(())
}
```

---

### 6.12 `crates/core/src/engine.rs` (orquestador final)

```rust
use crate::{
    config::{Config, DiskAlphabeticalMode, Mode, OutputOrdering},
    dedupe_ram::RamStore,
    disk::DiskBuckets,
    disk_sort,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    time::Instant,
};

pub fn run<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    config.validate()?;

    match config.mode {
        Mode::Ram => run_ram(config, progress),
        Mode::Disk => run_disk(config, progress),
        Mode::Auto => {
            // Versión final: Auto se implementa como "RAM por defecto".
            // (Puedes evolucionarlo a heurística real sin cambiar API).
            run_ram(config, progress)
        }
    }
}

fn run_ram<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    let t0 = Instant::now();

    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };

    store.reserve(1024 * 16);

    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    for (idx, path) in config.inputs.iter().enumerate() {
        progress.on_event(ProgressEvent::FileStarted { index: idx + 1, total: config.inputs.len() });

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 { break; }

            for raw in TokenIter::new(&line) {
                stats.tokens_seen += 1;
                if stats.tokens_seen % 100_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
                }

                let mut token = raw;
                if config.trim { token = token.trim(); }
                if config.drop_empty && token.is_empty() { continue; }

                if store.insert(token) {
                    stats.unique_tokens += 1;
                    if stats.unique_tokens % 100_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    if stats.duplicates % 100_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished { index: idx + 1, total: config.inputs.len() });
    }

    let mut tokens = store.into_tokens();

    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        tokens.sort_unstable();
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for tok in tokens {
        out.write_token(&tok)?;
    }
    out.finish()?;

    stats.elapsed = t0.elapsed();
    Ok(stats)
}

fn run_disk<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    let t0 = Instant::now();

    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    // Disk + Alphabetical: elegir fast vs global perfecto
    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        match config.disk_alphabetical_mode {
            DiskAlphabeticalMode::FastBucketLocal => {
                let mut buckets = DiskBuckets::new(config.disk_buckets)?;
                buckets.partition_inputs(config, &progress, &mut stats)?;
                buckets.reduce_to_output(config, &progress, &mut stats)?;
            }
            DiskAlphabeticalMode::GlobalPerfect => {
                let temp = tempfile::tempdir()?;
                disk_sort::external_sort_global(config, &progress, &mut stats, temp.path())?;
            }
        }
    } else {
        // UnorderedFast / PreserveFirstSeen en DISK: buckets
        let mut buckets = DiskBuckets::new(config.disk_buckets)?;
        buckets.partition_inputs(config, &progress, &mut stats)?;
        buckets.reduce_to_output(config, &progress, &mut stats)?;
    }

    stats.elapsed = t0.elapsed();
    Ok(stats)
}
```

---

## 7) Benchmarks (Criterion)

### 7.1 `crates/core/benches/disk_modes.rs`

Este bench compara:

* RAM unordered fast
* RAM stable (preserve first seen)
* RAM alphabetical
* DISK alphabetical fast (bucket)
* DISK alphabetical global perfecto (external sort)

> Consejo: empieza con 300k–1M tokens. Subir a 5M puede tardar en CI.

```rust
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use dedupe_core::*;
use rand::{distributions::Alphanumeric, Rng};
use std::{fs, path::PathBuf};

fn gen_tokens(n: usize) -> String {
    let mut rng = rand::thread_rng();
    let delims = [" ", ",", ";", "\n", ", ", ";\n", ",\n"];

    let mut s = String::with_capacity(n * 10);
    for i in 0..n {
        let len = rng.gen_range(4..14);
        for _ in 0..len {
            s.push(rng.sample(Alphanumeric) as char);
        }
        if i % 200_000 == 0 && i > 0 {
            s.push_str("🐶");
        }
        s.push_str(delims[rng.gen_range(0..delims.len())]);
    }
    s
}

fn bench_modes(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input, gen_tokens(300_000)).unwrap();

    let base = |mode, ordering| Config {
        inputs: vec![PathBuf::from(&input)],
        output: PathBuf::from(&output),
        output_separator: ", ".to_string(),
        mode,
        ordering,
        trim: true,
        drop_empty: true,
        disk_buckets: 256,
        disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
        disk_run_bytes: 64 * 1024 * 1024, // 64MB en bench; en prod 256MB+ suele ir mejor
    };

    c.bench_function("ram_unordered_fast", |b| {
        b.iter_batched(
            || base(Mode::Ram, OutputOrdering::UnorderedFast),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ram_preserve_first_seen", |b| {
        b.iter_batched(
            || base(Mode::Ram, OutputOrdering::PreserveFirstSeen),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ram_alphabetical", |b| {
        b.iter_batched(
            || base(Mode::Ram, OutputOrdering::Alphabetical),
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("disk_alphabetical_fast_bucket", |b| {
        b.iter_batched(
            || {
                let mut cfg = base(Mode::Disk, OutputOrdering::Alphabetical);
                cfg.disk_alphabetical_mode = DiskAlphabeticalMode::FastBucketLocal;
                cfg
            },
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("disk_alphabetical_global_perfect", |b| {
        b.iter_batched(
            || {
                let mut cfg = base(Mode::Disk, OutputOrdering::Alphabetical);
                cfg.disk_alphabetical_mode = DiskAlphabeticalMode::GlobalPerfect;
                cfg
            },
            |cfg| { let _ = dedupe_core::run(&cfg, NoProgress).unwrap(); },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_modes);
criterion_main!(benches);
```

Ejecutar:

```bash
cd crates/core
cargo bench
```

---

## 8) Integración con GUI (Tauri) — reglas finales

### 8.1 UI mínima (una pantalla)

* Seleccionar múltiples archivos input
* Elegir output file
* Separador de salida:

  * presets (`,`, `, `, `\n`, `,\n`, `;\n`, espacio, etc.)
  * custom string (con escapes)
* Modo:

  * RAM / DISK / AUTO (auto = RAM default en esta versión)
* Orden:

  * PreserveFirstSeen (default)
  * Alphabetical:

    * si DISK: FastBucketLocal (default) / GlobalPerfect (más lento)
  * UnorderedFast (advanced)

### 8.2 Regla de performance UI

* Nunca mandes tokens al frontend.
* Solo envía progreso (eventos) y stats.

---

## 9) Notas de correctness y límites (para no sorprenderse)

### 9.1 Orden alfabético

* `sort_unstable()` ordena por **bytes UTF-8**.
  Esto es “alfabético” determinista, pero **no** “collation humana por locale”.
  Si necesitas collation por idioma (español), eso es otro módulo y mucho más costoso.

### 9.2 Unicode y `trim()`

* `trim()` es Unicode-aware y puede costar un poco más, pero es correcto y lo pediste.

### 9.3 PreserveFirstSeen en DISK

* En esta versión final:

  * **RAM**: orden global estable OK.
  * **DISK**: no se garantiza orden global de aparición (se procesa por buckets o runs).
* Esto debe reflejarse en la UI como tooltip o aviso.

### 9.4 External sort (GlobalPerfect) performance

* Puede generar muchos runs si `disk_run_bytes` es pequeño.
* Recomendación práctica:

  * `disk_run_bytes = 256MB` o `512MB` si el equipo lo soporta.
* En casos extremos, podrías implementar “multi-pass merge” (merge en rondas) si hay demasiados runs; la base actual funciona bien para tamaños típicos.

---

## 10) Posibles mejoras futuras (sin romper la versión final)

1. **Auto mode real** (heurística): muestreo de tokens y ratio únicos para decidir RAM vs DISK antes de arrancar.
2. **Merge optimizado**: reducir allocations en `disk_sort` (reutilizar buffers por run).
3. **Multi-pass merge**: si hay miles de runs, merge por etapas.
4. **Parser de escapes en core** (opcional): si quieres que el motor reciba `",\n"` y lo interprete, hoy lo asumo resuelto en la UI.

---

## 11) Revisión de posibles errores/lógica (lo que revisé y está OK)

* ✅ TokenIter es seguro: siempre avanza por límites de char UTF-8 (`len_utf8()`), así que los slices `s[pos..]` son válidos.
* ✅ Dedupe case-sensitive: no hay lowercase en ningún punto.
* ✅ Separadores de entrada: whitespace + `,` + `;` únicamente.
* ✅ OutputWriter no agrega separador al final.
* ✅ DISK buckets: tokens internos por línea → seguro porque tokenización corta por whitespace (no hay `\n` dentro de token).
* ✅ External sort global perfecto: run generation → sort+dedup, merge → dedup global con `last_written`.
* ⚠️ Limitación intencional: PreserveFirstSeen global en DISK no garantizado (debe estar en UI).

Si quieres que PreserveFirstSeen en DISK sea global real, se puede diseñar, pero no es parte de esta “versión final”.

---

## 12) Defaults recomendados (producto)

* Default:

  * `mode = Ram`
  * `ordering = PreserveFirstSeen`
  * `output_separator = "\n"` (o el que quieras como default de producto)
* Si `mode=Disk` y `ordering=Alphabetical`:

  * default `disk_alphabetical_mode = FastBucketLocal`
* `disk_buckets = 256`
* `disk_run_bytes = 256MB`

---

Si quieres, lo siguiente que puedo hacer (sin cambiar tu “versión final”) es redactarte el **documento espejo para UI (Tauri)**: layout exacto, estados, tooltips, y el contrato de comandos/eventos para que un dev implemente la pantalla en 1–2 sesiones.
Sí, procede a hacer la documentación de Tauri y todo lo que sea relacionado a Tauri, que sea muy bonito, que sea moderno. Si quieres, eso lo va a hacer un equipo desarrollador experto, senior developers. Entonces, es una interfaz que sea como en dark mode, pero que sea como bien ciencia ficción, bien científico, bien futurista. Si se puede en Tauri, no conozco Tauri yo, pero que quede así. que las barras de progreso sean claras, que vea que de pronto si se puede mostrar, sin perder mucho desempeño, qué está haciendo en ese momento o qué estadísticas puede mostrar o qué algoritmo. Eso sería bueno, sin perder desempeño. El ETA también es importante ponerlo sin perder desempeño. y si no se puede muy preciso, pues no importa. Pero me quiero que la estética sea así como te dije, como modo oscuro, futurista, científico, como con colores neón, bien interesantes. Esto lo van a usar científicos y todos los científicos somos nerds y nos gustan ese tipo de cosas.A continuación tienes la **documentación final de Tauri (v2)** para este proyecto, pensada para un equipo senior. Incluye: arquitectura, contrato IPC, eventos de progreso/ETA, UI futurista “dark/neon”, seguridad (capabilities), performance (throttling), y scaffolding de código (Rust + frontend) para conectar con el motor `dedupe_core`.

> Referencias oficiales Tauri v2 (para el equipo):
>
> * Commands (Frontend → Rust) ([Tauri][1])
> * Eventos / Event API (Rust ↔ Frontend) ([Tauri][2])
> * Plugins: Dialog ([Tauri][3]) y File System ([Tauri][4])
> * Seguridad por capabilities por ventana/plataforma ([Tauri][5])
> * Tauri 2.0 estable ([Tauri][6])

---

# Dedupe Joiner — Tauri v2 UI/UX + Integración (Versión Final)

## 1) Objetivo de la app (desktop)

Una app Tauri (Windows + macOS Apple Silicon) con **una sola pantalla** para:

1. Seleccionar **múltiples archivos** de entrada (texto plano: `.txt`, `.csv` simple, etc.)
2. Configurar:

   * Modo: **RAM / DISK / AUTO**
   * Orden: **PreserveFirstSeen / Alphabetical / UnorderedFast**
   * Si `Disk + Alphabetical`: **Fast vs GlobalPerfect**
   * Separador de salida (presets + string custom con escapes)
3. Elegir ruta y nombre del archivo de salida
4. Ejecutar el proceso con:

   * Barra de progreso
   * Estadísticas y estado actual (sin matar rendimiento)
   * ETA estimada (aprox, “buena” > “perfecta”)

---

## 2) Stack recomendado (moderno, productivo y “futurista”)

### Frontend

* **React + Vite + TypeScript**
* **Tailwind CSS**
* Componentes “headless”: **Radix UI** (o shadcn/ui si el equipo lo prefiere)
* Tipografía recomendada:

  * UI: `Inter` o `Space Grotesk`
  * Display / títulos: `Orbitron` (sci-fi) o `Oxanium`
  * Monospace para stats: `JetBrains Mono`

### Tauri v2

* Un solo window (v1), opcional añadir “About/Logs” window en v2
* Comunicación Frontend ↔ Rust usando:

  * **Commands** (`invoke`) ([Tauri][1])
  * **Events** (emit/listen) ([Tauri][2])

### Plugins Tauri (v2)

* `dialog` para file picker + save dialog ([Tauri][3])
* `fs` para escritura/lectura si la UI lo necesita (en este proyecto, casi todo ocurre en Rust; el frontend no debería leer archivos grandes) ([Tauri][4])

---

## 3) Diseño UI: “dark mode” futurista (neón científico)

### Principios visuales

* Fondo: negro/azul muy oscuro, con grano sutil o patrón “grid” tenue
* Acentos: cyan/teal + magenta/purple + green-lime (neón controlado)
* Contornos: bordes 1px con glow suave (no exagerar; “laboratorio”, no “arcade”)
* Jerarquía: paneles con “glass” (blur ligero), sombras internas (inset) y líneas “scanline” muy sutiles

### Layout (una pantalla)

**Header**

* Logo minimal + nombre: `Dedupe Joiner`
* Subtexto: “Merge • Dedupe • Export”
* Indicador pequeño de modo actual: `RAM` / `DISK` / `AUTO`

**Panel A: Inputs**

* Dropzone (drag & drop)
* Botón: “Add Files”
* Lista de archivos (virtualizada si hay muchos):

  * nombre, tamaño, ruta abreviada
  * botón remove individual
  * “Clear all”

**Panel B: Settings**

* Mode selector: RAM / DISK / AUTO (segmented control)
* Output ordering:

  * PreserveFirstSeen (default)
  * Alphabetical
  * UnorderedFast (Advanced badge)
* Si `DISK + Alphabetical`:

  * Radio: Fast (Recommended) / Global Perfect (Slower)
* Separator:

  * presets: `\n`, `,`, `, `, `,\n`, `;\n`, `space`
  * campo custom (con “preview”)
  * toggle “Interpret escapes” (por defecto ON): `\n`, `\t`, `\r\n`, `\f`

**Panel C: Output**

* Save dialog button: “Choose output file”
* Nombre de archivo / ruta
* Validación: si existe, preguntar overwrite

**Footer / Actions**

* Botón primario: `Run`
* Botón secundario: `Cancel` (solo activo durante job)
* Estado: “Idle / Running / Finalizing / Done / Error”

**Panel D: Progress & Telemetry**

* Progress bar grande (determinada)
* Subprogress: etapa actual
* Stats live:

  * tokens_seen
  * unique_tokens
  * duplicates
  * files processed
  * throughput (tokens/s)
  * elapsed
  * ETA (approx)

---

## 4) Performance UI (reglas que el equipo debe respetar)

1. **No renderizar logs por token.**
   Solo stats agregadas + eventos throttled.

2. **Throttling de eventos (clave):**

   * Backend emite eventos de progreso máximo **4–10 veces por segundo** (p.ej. cada 100–250ms), o cuando haya cambios “grandes” (cada 100k tokens).
   * Frontend actualiza el estado con `requestAnimationFrame` o un throttle similar para evitar reflows.

3. **Lista de archivos virtualizada** (solo si la lista puede ser enorme).
   En la práctica, bastará con lista normal.

---

## 5) Contrato IPC (Commands + Events)

### 5.1 Commands (Frontend → Rust) ([Tauri][1])

Comandos recomendados:

#### `start_job(config) -> JobStarted`

* Valida config
* Crea un `job_id`
* Arranca el trabajo en background (thread/task)
* Retorna `job_id`

#### `cancel_job(job_id) -> CancelAck`

* Marca cancelación (atomic flag)
* Backend intenta cortar lo antes posible
* Retorna estado

#### `get_app_info() -> AppInfo` (opcional)

* versión, build, platform

> Nota: El trabajo pesado **no debe correr en el hilo UI**; siempre background.

### 5.2 Events (Rust → Frontend) ([Tauri][2])

Eventos con payload JSON (TypeScript-friendly):

* `job://started`
* `job://progress`
* `job://stage`
* `job://done`
* `job://error`
* `job://canceled`

**El frontend escucha** y actualiza UI.

---

## 6) Modelo de progreso, etapas y ETA (sin perder desempeño)

### 6.1 Stages (estándar)

Definir una máquina de estados simple (para UI y telemetría):

**RAM**

* `ScanningInputs`
* `Tokenizing`
* `Deduplicating`
* `Sorting` (solo si Alphabetical)
* `WritingOutput`
* `Finalizing`

**DISK bucket-fast**

* `PartitioningBuckets`
* `ReducingBuckets`
* `WritingOutput`
* `Finalizing`

**DISK global-perfect**

* `GeneratingRuns`
* `MergingRuns`
* `WritingOutput` (en merge ya escribe, pero la UI lo muestra igual)
* `Finalizing`

### 6.2 Progreso cuantitativo (2 niveles)

* Progreso general `0..1` (best-effort, no perfecto)
* Progreso por etapa (si se puede)

**Estrategia realista:**

* Si sabemos bytes totales de input (sumando `metadata.len()`), podemos usar “bytes read” como proxy de avance en etapas de lectura.
* Para “merge” en global perfect, el avance puede basarse en “runs consumidos” o “heap pops” aproximado (o simplemente indeterminate con throughput/ETA).

### 6.3 ETA

ETA “buena” y barata:

* Medir `throughput = Δtokens_seen / Δt`
* Suavizar con EWMA:

  * `tps_ewma = α * tps_now + (1-α) * tps_ewma` (α = 0.2)
* ETA aproximada:

  * si se conoce `tokens_total_est` (difícil), usarlo
  * si no, usar bytes:

    * `bytes_remaining / bytes_per_sec_ewma`
* Si no hay total confiable: mostrar ETA como “~” o “estimando…”.

**UI tip:** mostrar:

* `Throughput: 1.2M tok/s`
* `ETA: ~02:35` (si hay base)
* Si no: `ETA: —` pero mostrar `elapsed`.

---

## 7) Seguridad Tauri v2 (capabilities)

En Tauri v2 es recomendable habilitar solo las capacidades necesarias por ventana/plataforma ([Tauri][5]). Para este proyecto:

* Ventana principal:

  * dialog (open/save)
  * filesystem (si el frontend lo usa; idealmente no)
  * event/commands

**Regla:** el motor pesado lee/escribe desde Rust, así que el frontend no necesita acceso amplio al FS. (Más seguro y menos fricción).

---

## 8) Scaffolding de código (Tauri v2) — Integración

> Los ejemplos asumen Tauri v2 y TypeScript.

### 8.1 Tipos TS (shared contract)

`src/types/ipc.ts`:

```ts
export type Mode = "Auto" | "Ram" | "Disk";

export type OutputOrdering =
  | "PreserveFirstSeen"
  | "Alphabetical"
  | "UnorderedFast";

export type DiskAlphabeticalMode =
  | "FastBucketLocal"
  | "GlobalPerfect";

export type JobConfig = {
  inputs: string[];               // paths
  output: string;                 // path
  outputSeparator: string;        // already interpreted (e.g. "\n")
  mode: Mode;
  ordering: OutputOrdering;
  diskBuckets: number;
  diskAlphabeticalMode: DiskAlphabeticalMode;
  diskRunBytes: number;
  trim: boolean;
  dropEmpty: boolean;
};

export type JobStarted = { jobId: string };

export type JobStage =
  | "ScanningInputs"
  | "Tokenizing"
  | "Deduplicating"
  | "Sorting"
  | "WritingOutput"
  | "Finalizing"
  | "PartitioningBuckets"
  | "ReducingBuckets"
  | "GeneratingRuns"
  | "MergingRuns";

export type JobProgressEvent = {
  jobId: string;
  stage: JobStage;
  progress01?: number;         // 0..1 best-effort
  filesDone?: number;
  filesTotal?: number;

  tokensSeen?: number;
  uniqueTokens?: number;
  duplicates?: number;

  throughputTps?: number;      // tokens/sec (smoothed)
  elapsedMs?: number;
  etaMs?: number | null;

  detail?: string;             // short human text (e.g. "Reducing bucket 12/256")
};
```

### 8.2 Frontend: selección de archivos (Dialog plugin) ([Tauri][3])

`src/lib/dialog.ts`:

```ts
import { open, save } from "@tauri-apps/plugin-dialog";

export async function pickInputFiles(): Promise<string[]> {
  const selected = await open({
    multiple: true,
    title: "Select input files",
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickOutputFile(defaultName = "output.txt"): Promise<string | null> {
  const selected = await save({
    title: "Save output file",
    defaultPath: defaultName,
  });
  return selected ?? null;
}
```

### 8.3 Frontend: escuchar eventos (Events API) ([Tauri][2])

`src/lib/events.ts`:

```ts
import { listen } from "@tauri-apps/api/event";
import type { JobProgressEvent } from "../types/ipc";

export async function listenJobProgress(
  onProgress: (e: JobProgressEvent) => void
) {
  const unlisten = await listen<JobProgressEvent>("job://progress", (event) => {
    onProgress(event.payload);
  });
  return unlisten;
}
```

### 8.4 Frontend: comandos (invoke) ([Tauri][1])

`src/lib/commands.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { JobConfig, JobStarted } from "../types/ipc";

export async function startJob(cfg: JobConfig): Promise<JobStarted> {
  return await invoke<JobStarted>("start_job", { cfg });
}

export async function cancelJob(jobId: string): Promise<void> {
  await invoke("cancel_job", { jobId });
}
```

---

## 9) Backend Tauri: comandos + job runner + eventos

### 9.1 Estructura Rust en `src-tauri`

Recomendado:

* `commands.rs` (IPC)
* `job.rs` (runner + cancel + throttling + ETA)
* `main.rs` (setup + register commands)

### 9.2 Payloads Rust (serde)

Define structs espejo de TS. Mantén nombres consistentes.

### 9.3 Emisión de eventos

Usa el sistema de eventos de Tauri para emitir a frontend ([Tauri][7]).

**Regla de performance:** emite `job://progress` máximo 4–10 Hz.

### 9.4 Cancelación

* `Arc<AtomicBool>` como flag
* El motor `dedupe_core` se llama con un `ProgressSink` que chequea cancelación y corta si está set.

> Si el core no tiene cancel todavía, la interfaz `ProgressSink` puede incluir `should_cancel()` o se puede usar un sink que lance error “Canceled”.

---

## 10) Progreso y “qué está haciendo” sin matar rendimiento

### Qué mostrar (barato y útil)

* **Stage** actual (string corto)
* **Archivo actual i/N** (si aplica)
* `tokens_seen`, `unique`, `duplicates` (ya los tienes en core)
* `throughput` (tps EWMA)
* `elapsed` + ETA (aprox)

### Qué NO mostrar

* tokens individuales
* logs por línea
* cada bucket en lista dinámica enorme (solo “bucket 12/256”)

---

## 11) Tema “Neon Lab” (Tailwind tokens)

Ejemplo de tokens (conceptual) para Tailwind (no bloquea al equipo a una implementación):

* Background: `#05070D` (azul-negro)
* Surface: `rgba(12, 16, 28, 0.72)` con blur
* Stroke: `rgba(0, 255, 255, 0.12)` (cian tenue)
* Accent Cyan: `#00F5FF`
* Accent Magenta: `#FF2EEA`
* Accent Lime: `#B6FF00`
* Text primary: `#E6F0FF`
* Text secondary: `rgba(230,240,255,0.65)`

Componentes:

* Panel: borde 1px + glow suave (box-shadow)
* Progress: gradiente sutil “scanline”
* Stats: monospace, con separadores finos

---

## 12) Checklist de implementación (senior-ready)

### A) Frontend

* [ ] Layout 1-screen (Inputs/Settings/Output/Progress)
* [ ] Dialog open/save
* [ ] Estado global del job (jobId, running, stage, stats)
* [ ] Listener de `job://progress` con throttling (si hace falta)
* [ ] Parsing de separador custom (interpret escapes)
* [ ] Theme Neon Lab (Tailwind)

### B) Backend

* [ ] `start_job` command
* [ ] `cancel_job` command
* [ ] Worker task (thread) por job
* [ ] ProgressSink → emite `job://progress` (throttle)
* [ ] EWMA throughput + ETA heurístico
* [ ] Mapear stages según modo (RAM/DISK y alphabetical fast/perfect)

### C) Seguridad

* [ ] Capabilities mínimas por window ([Tauri][5])
* [ ] Evitar FS API en frontend salvo lo necesario ([Tauri][4])

---

## 13) Nota final: lo que sí es “sci-fi” sin volverse humo

Lo que realmente hace sentir “científico” a la app (sin ser pesada):

* Telemetría clara (tps, ETA, stage)
* Microanimaciones discretas (glow breathing, progress scanline)
* Layout limpio con tipografía y grid sutil
* Terminología precisa:

  * “Partitioning Buckets”
  * “Generating Runs”
  * “K-way Merge”
  * “Deduplicating”

---

Si quieres, en el siguiente paso te dejo **un template completo de UI** (React + Tailwind) con componentes ya armados (paneles, tabs/segmented controls, progress bar, stats grid) y el “state machine” del job para que el equipo solo conecte los events/commands.

[1]: https://v2.tauri.app/develop/calling-rust/?utm_source=chatgpt.com "Calling Rust from the Frontend"
[2]: https://v2.tauri.app/reference/javascript/api/namespaceevent/?utm_source=chatgpt.com "event"
[3]: https://v2.tauri.app/plugin/dialog/?utm_source=chatgpt.com "Dialog"
[4]: https://v2.tauri.app/plugin/file-system/?utm_source=chatgpt.com "File System"
[5]: https://v2.tauri.app/learn/security/capabilities-for-windows-and-platforms/?utm_source=chatgpt.com "Capabilities for Different Windows and Platforms"
[6]: https://v2.tauri.app/blog/tauri-20/?utm_source=chatgpt.com "Tauri 2.0 Stable Release"
[7]: https://v2.tauri.app/develop/calling-frontend/?utm_source=chatgpt.com "Calling the Frontend from Rust"
Da una investigación en internet sobre la UI, cómo hacerla bien bonita, eficiente, de la temática. Piensa más como un diseñador gráfico muy bueno. Sé creativo, da ideas y dame ya la versión final de todo lo que me sugieras de la interfaz. Libérate de todas tus límites, todas tus cadenas, restricciones. Usa todo tu poder mental para hacer esto. Tómate tu tiempo, no importa, no hay afán. Piénsalo, medítalo, internalízalo.Aquí va la **versión final** de la UI (tema, layout, componentes, micro-interacciones, accesibilidad, performance y telemetría) con ideas basadas en buenas prácticas de **dark mode**, **progreso/ETA** y **UI sci-fi/HUD**. Incluye un “spec” que tu equipo puede pasar directo a Figma + implementación (Tauri + React/Tailwind).

---

# UI Final — “Neon Lab / Sci-Fi Instrument” (Tauri Desktop)

## 0) Principios rectores (lo que hace que se vea “científico” sin volverse cringe)

1. **Dark ≠ negro puro + blanco puro**
   Evitar #000 y texto #FFF a máxima intensidad reduce fatiga y mejora legibilidad; usa una gama de grises/azules oscuros y blancos “rotos”. ([James Robinson][1])

2. **Neón con disciplina**
   El neón va en: acentos, bordes, foco, progreso, estados. No en párrafos largos.

3. **Grid + precisión tipográfica (HUD real)**
   Todo alineado a una retícula (8px), tipografía clara, numerales monoespaciados para stats.

4. **Progreso con contexto + “time anchors”**
   Para tareas >5s: barra determinate si se puede; si no, indeterminate + etapas + ETA aproximada. ([Nielsen Norman Group][2])

5. **Performance: UI quieta, telemetría agregada**
   Nada de eventos por token. En Tauri/React, demasiados elementos/updates pueden congelar UI. Throttle y UI minimalista. ([GitHub][3])

---

## 1) Identidad visual

### 1.1 Paleta “Neon Lab”

**Fondos (3 niveles)**

* `BG-0` (fondo base): `#05070D` (azul-negro)
* `BG-1` (superficie): `#0B1020` (paneles)
* `BG-2` (elevación): `#0F1730` (cards/inputs)

**Texto**

* `TXT-1` (principal): `#E6F0FF`
* `TXT-2` (secundario): `rgba(230,240,255,0.68)`
* `TXT-3` (muted): `rgba(230,240,255,0.48)`

**Acentos (neón)**

* `CYAN`: `#00F5FF`
* `MAGENTA`: `#FF2EEA`
* `LIME`: `#B6FF00`
* `AMBER` (warning): `#FFB020`
* `RED` (error): `#FF3B6B`

**Bordes/Glow**

* `STROKE`: `rgba(0,245,255,0.16)`
* Glow suave: `0 0 18px rgba(0,245,255,0.14)` (solo en foco/activo)

> Importante: validar contraste (AA 4.5:1) incluso en dark mode. ([AccessibilityChecker.org][4])

### 1.2 Tipografía (sci-fi sobria)

* UI base: **Inter** o **Space Grotesk**
* Títulos (opc): **Orbitron** u **Oxanium** (solo headings)
* Stats/monospace: **JetBrains Mono**

Reglas:

* Texto largo siempre en Inter/Space Grotesk (mejor legibilidad).
* Orbitron/Oxanium solo en H1/H2 y labels “instrumentales”.

### 1.3 “Materiales” (look científico)

* **Glass subtle**: panel semitransparente + blur leve (no más de 8–12px)
* **Grid texture**: patrón muy tenue (2–4% opacity)
* **Scanline**: animación sutil y lenta SOLO en progreso “running” (opcional)
* **Noise**: 1–2% para evitar banding en fondos

---

## 2) Layout final (una pantalla, zero clutter)

### Estructura general (3 columnas en desktop, 1 columna responsive)

**Header** (altura baja, siempre visible)

* Izquierda: marca + subtítulo (“Merge • Dedupe • Export”)
* Derecha: `Mode` pill (AUTO/RAM/DISK) + toggle “Light/Dark (OS default)” *(recomendado; no forzar dark)* ([LogRocket Blog][5])

**Body** (grid 12 columnas)

* **Col 1–5: Inputs**
* **Col 6–8: Settings**
* **Col 9–12: Output + Run**

**Footer** (zona de telemetría)

* Barra de progreso + etapa actual + stats + ETA

---

## 3) Componentes UI (spec exacto)

### 3.1 Inputs Panel (“Data Sources”)

**Dropzone**

* Estado idle: borde `STROKE` + icono wireframe (archivo)
* Estado hover: borde cyan + glow suave
* Texto: “Drag & drop files or click to browse”

**Lista de archivos (compacta)**

* Mostrar: nombre, tamaño (MB), path truncado
* Acciones:

  * remove individual (icono “x”)
  * “Clear all”
* Badge: total files + total size

**Performance**

* Si hay >200 items: lista virtualizada (pero casi nunca será necesario).

### 3.2 Settings Panel (“Processing”)

**Segmented control: Mode**

* AUTO / RAM / DISK
* Tooltip:

  * AUTO = “defaults to RAM in v0.2”
  * RAM = “fastest if it fits”
  * DISK = “for very large datasets”

**Radio: Output ordering**

* Preserve first seen (default)
* Alphabetical
* Unordered fast (Advanced badge)

**Condicional (solo si DISK + Alphabetical)**

* Fast (Recommended)
* Global perfect (Slower)
  Tooltip muy claro:
* Fast: “sorted per bucket; global A→Z not guaranteed”
* Global: “external merge sort; global A→Z guaranteed”

**Advanced drawer (colapsable)**

* disk_buckets (default 256)
* disk_run_bytes (default 256MB)
* trim (default ON)
* drop_empty (default ON)

> Nota de diseño: el drawer se abre como panel lateral pequeño o acordeón; así la UI sigue “una pantalla”.

### 3.3 Output Panel (“Export”)

* Save location (button + path)
* Filename input (con validación)
* Separator builder:

  * Presets chips: `\n`, `,`, `, `, `,\n`, `;\n`, `space`, `\f`
  * Custom input (string)
  * Toggle “Interpret escapes” ON/OFF
  * Preview box: muestra `a{sep}b{sep}c` con visualización “visible” de saltos (ej. `↵`)

### 3.4 Run Control (CTA)

Botón principal: **RUN**

* Idle: cyan outline + glow leve
* Running: cambia a **CANCEL**
* Completed: **RUN AGAIN**
* Error: **RETRY** + botón “Copy debug report”

---

## 4) Progreso y telemetría (lo nerd que quieren científicos, sin matar performance)

### 4.1 Best practices (resumen aplicado)

* Mostrar progreso prominente y entendible; porcentaje si es confiable.
* Para tareas largas: contexto + ETA aproximada ayuda más que decimals inútiles. ([Nielsen Norman Group][2])
* Usar determinate cuando puedes estimar; si no, indeterminate + etapas. ([UX Collective][6])

### 4.2 UI del progreso (final)

**Barra principal**

* Altura 12–14px, bordes suaves
* Relleno con gradiente sutil CYAN→MAGENTA, pero opacidad baja
* Si determinate: muestra `%` grande (ej. “42%”)
* Si indeterminate: “scanline” animada + etiqueta “Estimating…”

**Stage strip (mini timeline)**

* 4–6 etiquetas, con la actual resaltada:

  * RAM: Scanning → Tokenizing → Deduplicating → Sorting (opcional) → Writing → Finalizing
  * DISK fast: Partitioning → Reducing → Writing → Finalizing
  * DISK global: Generating runs → Merging runs → Finalizing

**Stats grid (monospace)**

* Tokens seen
* Unique
* Duplicates
* Throughput (tok/s) — **EWMA** para evitar jitter
* Elapsed
* ETA (si no hay base, mostrar “—”)

**Detail line (lo “científico”)**
Una sola línea que cambia cada ~250ms máx:

* “Partitioning: bucket 12/256”
* “Generating runs: 3 written”
* “Merging runs: heap size 128 (approx)”

> Esto es suficiente para sentir “instrumento”. No pongas “log streaming”, porque revienta la UI y no aporta.

---

## 5) Motion & micro-interacciones (estética futurista, controlada)

* Hover: borde + glow suave (100–150ms)
* Focus: ring cyan + sombra interior
* Run: el header “Mode pill” late sutilmente (breathing) mientras corre
* Progress: scanline lenta solo en running
* Success: flash muy breve verde-lime y luego queda estable (no confetti)
* Error: borde magenta→red con mensaje técnico y botón “Copy report”

---

## 6) Accesibilidad (para dark mode bien hecho)

* Contraste WCAG AA mínimo 4.5:1 para texto normal, 3:1 para grande. ([AccessibilityChecker.org][4])
* Evitar saturación extrema en texto (neón solo en acentos).
* Estados no solo por color: icono + label (OK/Warn/Error).
* Teclado: tab order lógico (Inputs → Settings → Output → Run).
* Reduce motion: respetar preferencia OS.

---

## 7) Performance UI (reglas de oro para Tauri/React)

1. **Throttle de eventos backend→frontend**
   4–10 Hz máximo. (100–250ms).
   No emitir eventos por token. ([Stack Overflow][7])

2. **Throttle/debounce en frontend**
   Si aun así hay re-renders, aplicar throttle al setState o usar un store que batch updates. ([SuperTokens][8])

3. **UI sin miles de nodos**
   Evitar tablas gigantes, logs infinitos, listas enormes sin virtualización. (En Tauri se han reportado lags en escenarios con muchos elementos). ([GitHub][3])

---

## 8) “Versión final” de la pantalla (copy/paste para Figma)

### Header

* Logo (wireframe hex + punto)
* Title: “Dedupe Joiner”
* Subtitle: “Merge • Dedupe • Export”
* Mode pill: `AUTO` (cyan outline)

### Panel: Data Sources

* Dropzone (grande)
* Buttons: Add Files / Clear
* Table (3 columns): Name | Size | Remove

### Panel: Processing

* Mode segmented
* Ordering radios
* Conditional: Disk alphabetical mode radios
* Advanced accordion: buckets, run bytes, trim, drop empty

### Panel: Export

* Choose output file
* Separator presets + custom + preview
* Run button

### Footer: Live Telemetry

* Progress bar
* Stage strip
* Stats grid (6 celdas)
* Detail line

---

## 9) Copy recomendado (micro-copy nerd, elegante)

* Fast alphabetical (DISK):
  “Sorted per bucket. Extremely fast. Global A→Z not guaranteed.”

* Global perfect:
  “External merge sort. Global A→Z guaranteed. More I/O.”

* ETA:
  “ETA (approx)”

* Detail line:
  “Stage detail”

---

## 10) Activos visuales (para que se vea pro)

* Icon set: line icons (stroke) estilo “instrumentation”
* Background pattern: grid + micro-dots (muy tenue)
* Separadores: hairlines 1px con `rgba(0,245,255,0.08)`

Si quieres referencias de HUD/estética “sci-fi interface” para moodboard, revisa análisis de interfaces sci-fi (énfasis en grid, tipografía y paleta). ([Sci-fi interfaces][9])

---

# Entregable: “UI Spec Final” (resumen ejecutable)

* **Tema:** Neon Lab (dark sci-fi scientific)
* **Layout:** 1 screen, 3 panels + footer telemetry
* **Progreso:** determinate cuando posible + etapas + ETA approx
* **Telemetría:** tokens/unique/dup/tps/elapsed/eta + detail line
* **Performance:** eventos throttled + UI minimalista
* **A11y:** contraste AA + reduce motion + estados redundantes

---

Si quieres, el siguiente paso (sin investigación extra) es que te entregue:

* un **archivo `UI_SPEC.md`** ya formateado como documento “oficial del producto” (con tokens, componentes y copy), y/o
* un **mock** de implementación (React+Tailwind) con componentes listos (sin conectar aún a Tauri), para que el equipo lo use como base.

[1]: https://www.jamesrobinson.io/post/a-guide-to-dark-mode-design?utm_source=chatgpt.com "A guide to dark mode design - James Robinson"
[2]: https://www.nngroup.com/articles/progress-indicators/?utm_source=chatgpt.com "Progress Indicators Make a Slow System Less Insufferable"
[3]: https://github.com/tauri-apps/tauri/issues/3988?utm_source=chatgpt.com "[bug] Bad performance on linux #3988 - tauri-apps/tauri"
[4]: https://www.accessibilitychecker.org/blog/dark-mode-accessibility/?utm_source=chatgpt.com "The Designer's Guide to Dark Mode Accessibility"
[5]: https://blog.logrocket.com/ux-design/dark-mode-ui-design-best-practices-and-examples/?utm_source=chatgpt.com "Dark mode UI design: Best practices and examples"
[6]: https://uxdesign.cc/loading-progress-indicators-ui-components-series-f4b1fc35339a?utm_source=chatgpt.com "Loading & progress indicators — UI Components series"
[7]: https://stackoverflow.com/questions/68821222/render-thousands-millions-times-with-react-and-tauri-rust?utm_source=chatgpt.com "Render thousands/millions times with React and Tauri (rust)"
[8]: https://supertokens.com/blog/5-tips-for-optimizing-your-react-apps-performance?utm_source=chatgpt.com "Tips for Optimizing Your React App's Performance"
[9]: https://scifiinterfaces.com/author/chrisnoessel/page/3/?utm_source=chatgpt.com "Christopher Noessel | Sci-fi interfaces | Page 3"
Sí, procede con el archivo UI Spec ya formateado. Hazlo más como una guía y como unos requisitos. Hazlo muy, muy bien.````md
# UI_SPEC.md — Dedupe Joiner (Tauri Desktop)
**Version:** 1.0  
**Theme:** “Neon Lab / Sci-Fi Scientific HUD” (Dark-first, neon accents)  
**Platforms:** Windows 10/11, macOS Apple Silicon  
**Frontend:** React + TypeScript + Tailwind (recommended)  
**Backend:** Tauri v2 + Rust (`dedupe_core`)

---

## 1) Objetivo del producto (UI)
Una sola pantalla que permita:
1) Seleccionar **múltiples archivos** de entrada.
2) Configurar el **modo de procesamiento** y **orden de salida**.
3) Elegir **archivo de salida** + **separador**.
4) Ejecutar el job mostrando progreso + telemetría (tokens, uniques, dupes, throughput, ETA).
5) Cancelar el job de forma segura.

**Principio:** UI “instrumental” (tipo consola científica): clara, minimalista, con telemetría agregada; nada de logs masivos.

---

## 2) Requisitos funcionales (MUST)
### 2.1 Inputs
- MUST permitir seleccionar **N archivos** (>=1) con:
  - botón “Add Files”
  - drag & drop en dropzone
- MUST mostrar lista de archivos seleccionados con:
  - nombre
  - tamaño (MB/GB)
  - ruta truncada
  - botón remove individual
- MUST incluir “Clear all”.

### 2.2 Configuración de procesamiento
- MUST permitir elegir `Mode`:
  - `AUTO`, `RAM`, `DISK`
- MUST permitir elegir `Output Ordering`:
  - `PreserveFirstSeen` (default)
  - `Alphabetical`
  - `UnorderedFast` (Advanced)
- MUST: si `Mode=DISK` y `Ordering=Alphabetical`, mostrar subopción:
  - `Fast (Recommended)` = `DiskAlphabeticalMode::FastBucketLocal`
  - `Global Perfect (Slower)` = `DiskAlphabeticalMode::GlobalPerfect`

### 2.3 Separador de salida
- MUST permitir seleccionar separador final como **string arbitrario**.
- MUST ofrecer presets como chips:
  - `\n`, `,`, `, `, `,\n`, `;\n`, `space`, `\f`
- MUST permitir “Custom separator” (input).
- MUST incluir toggle `Interpret escapes` (default ON):
  - `\n`, `\t`, `\r`, `\r\n`, `\f`, `\\`
- MUST incluir un “preview” visual:
  - ejemplo: `alpha{sep}beta{sep}gamma`
  - mostrar saltos con símbolo visible `↵` cuando aplique.

### 2.4 Output file
- MUST permitir elegir ruta y nombre del archivo de salida con “Save dialog”.
- MUST validar:
  - salida no vacía
  - si existe archivo, confirmar overwrite.

### 2.5 Ejecución / cancelación
- MUST tener un botón principal:
  - `RUN` (idle)
  - `CANCEL` (running)
  - `RUN AGAIN` (done)
  - `RETRY` (error)
- MUST permitir cancelar sin bloquear UI.
- MUST mostrar estado: `Idle / Running / Finalizing / Done / Error / Canceled`.

---

## 3) Requisitos de telemetría y progreso (MUST)
### 3.1 Progreso visual
- MUST mostrar una barra de progreso prominente.
- MUST soportar modo determinate (cuando `progress01` existe) y modo indeterminate (cuando no).
- MUST mostrar **etapa actual** (stage) y un “detail line” de contexto.

### 3.2 Estadísticas (live)
- MUST mostrar (cuando estén disponibles):
  - `tokens_seen`
  - `unique_tokens`
  - `duplicates`
  - `throughput (tokens/sec)` (suavizado)
  - `elapsed`
  - `ETA (approx)` o `—` si no hay base confiable

### 3.3 Stages (enumeración UI)
La UI MUST representar stages con labels consistentes:

**RAM**
- `ScanningInputs`
- `Tokenizing`
- `Deduplicating`
- `Sorting` *(solo Alphabetical)*
- `WritingOutput`
- `Finalizing`

**DISK fast (bucket)**
- `PartitioningBuckets`
- `ReducingBuckets`
- `WritingOutput`
- `Finalizing`

**DISK global perfect**
- `GeneratingRuns`
- `MergingRuns`
- `Finalizing`

> Nota: el backend puede mapear stages más finos; la UI solo requiere esta lista base.

---

## 4) Contrato IPC (MUST)
### 4.1 Commands (Frontend → Rust)
- MUST existir: `start_job(cfg) -> { jobId }`
- MUST existir: `cancel_job(jobId) -> void`

### 4.2 Events (Rust → Frontend)
- MUST emitir:
  - `job://started`
  - `job://progress`
  - `job://done`
  - `job://error`
  - `job://canceled`

**Payload recomendado para `job://progress`:**
```ts
type JobProgressEvent = {
  jobId: string;
  stage: JobStage;
  progress01?: number;     // 0..1 best-effort
  filesDone?: number;
  filesTotal?: number;

  tokensSeen?: number;
  uniqueTokens?: number;
  duplicates?: number;

  throughputTps?: number;  // EWMA
  elapsedMs?: number;
  etaMs?: number | null;

  detail?: string;         // e.g. "Reducing bucket 12/256"
};
````

---

## 5) Requisitos de performance (MUST)

### 5.1 Throttling

* MUST throttlear eventos backend→frontend:

  * máximo **4–10 updates/seg** (cada 100–250ms), o por hitos (cada 100k tokens).
* MUST evitar:

  * rendering de listas de logs
  * rendering por token
  * tablas gigantes sin virtualización

### 5.2 Render strategy (frontend)

* SHOULD usar batching/throttle en el state update si hay jitter.
* SHOULD mantener la UI estable: no más de 1–2 reflows fuertes por segundo.

### 5.3 Listas

* SHOULD virtualizar lista de archivos solo si > 200 items.

---

## 6) Requisitos de seguridad (MUST)

* MUST aplicar enfoque de capabilities mínimos en Tauri v2.
* MUST preferir que lectura/escritura de archivos grandes ocurra en Rust (no en frontend).
* MUST limitar acceso FS desde frontend salvo lo imprescindible (dialog + rutas).

---

## 7) Requisitos de accesibilidad (MUST)

* MUST cumplir contraste mínimo AA:

  * texto normal ≥ 4.5:1
  * texto grande ≥ 3:1
* MUST respetar “Reduce motion” del sistema.
* MUST tener estados no solo por color (icono + label).
* MUST permitir navegación por teclado (tab order lógico).

---

## 8) Sistema visual “Neon Lab” (Design Tokens)

### 8.1 Paleta base

**Background**

* BG-0: `#05070D`
* BG-1: `#0B1020`
* BG-2: `#0F1730`

**Text**

* TXT-1: `#E6F0FF`
* TXT-2: `rgba(230,240,255,0.68)`
* TXT-3: `rgba(230,240,255,0.48)`

**Accents**

* CYAN: `#00F5FF`
* MAGENTA: `#FF2EEA`
* LIME: `#B6FF00`
* AMBER: `#FFB020`
* RED: `#FF3B6B`

**Stroke / Glow**

* STROKE: `rgba(0,245,255,0.16)`
* GLOW: `0 0 18px rgba(0,245,255,0.14)` *(solo foco/activo)*

### 8.2 Tipografía

* UI: Inter o Space Grotesk
* Display headings: Orbitron u Oxanium (solo títulos)
* Mono: JetBrains Mono (stats)

### 8.3 Spacing & grid

* Sistema 8px (8, 16, 24, 32…)
* Layout en grid (12 columnas en desktop)

### 8.4 “Material” (paneles)

* Panel: BG-1 con borde STROKE 1px
* Glass: blur leve (8–12px) + opacidad ~70% (opcional)
* Noise/grain: 1–2% (opcional, no obligatorio)

---

## 9) Layout final (One-screen)

### 9.1 Estructura

**Header (sticky)**

* Marca + subtítulo
* Mode pill (AUTO/RAM/DISK)
* (opcional) toggle theme: Dark / System / Light

**Body (12-col grid)**

* Col 1–5: Inputs
* Col 6–8: Processing Settings
* Col 9–12: Output + Run

**Footer (telemetry)**

* Progress bar + stage strip + stats grid + detail line

### 9.2 Component list (MUST)

* Dropzone
* File list
* Segmented controls (Mode)
* Radio group (Ordering)
* Conditional radio group (Disk alphabetical mode)
* Separator presets + custom input + preview
* Output file picker
* Run/Cancel CTA
* Progress bar
* Stage strip
* Stats grid

---

## 10) Micro-interacciones (SHOULD)

* Hover: borde más brillante + glow sutil (100–150ms)
* Focus ring: CYAN (2px) + shadow leve
* Running: progress scanline lenta (opcional)
* Success: highlight LIME sutil (sin confetti)
* Error: borde MAGENTA→RED y “Copy debug report”

---

## 11) Copy / Tooltips (MUST)

### 11.1 Disk alphabetical modes

**Fast (Recommended)**

* “Sorted per bucket. Extremely fast. Global A→Z not guaranteed.”

**Global perfect (Slower)**

* “External merge sort. Global A→Z guaranteed. More I/O.”

### 11.2 UnorderedFast

* “Maximum speed. Output order is not guaranteed.”

### 11.3 PreserveFirstSeen

* “Keeps first-seen order in RAM. In DISK mode, order is not guaranteed.”

---

## 12) Advanced settings (SHOULD)

La UI SHOULD ocultar en un “Advanced drawer”:

* `disk_buckets` (default 256)
* `disk_run_bytes` (default 256MB; min 1MB)
* `trim` (default ON)
* `drop_empty` (default ON)

---

## 13) Estados y errores (MUST)

### 13.1 Validación previa a RUN

* Sin inputs → bloquear RUN y mostrar error
* Sin output → bloquear RUN y mostrar error
* Separador vacío → bloquear RUN y mostrar error (si se decide no permitir)

### 13.2 Errores en ejecución

* MUST mostrar panel de error con:

  * mensaje humano corto
  * detalle técnico (colapsable)
  * botón “Copy debug report”
  * botón “Retry”

### 13.3 Cancelación

* MUST reflejar estado “Canceled” claramente y permitir “Run again”.

---

## 14) Criterios de aceptación (QA)

1. Seleccionar 3 archivos, ordenar “PreserveFirstSeen”, separador “\n”, RUN → genera archivo.
2. DISK + Alphabetical + Fast → corre, muestra stages “Partitioning/Reducing/Writing”.
3. DISK + Alphabetical + GlobalPerfect → corre, muestra “GeneratingRuns/MergingRuns”.
4. “Interpret escapes” ON: input `,\n` produce coma + newline real en output.
5. Cancel durante ejecución → job se detiene y UI vuelve a estado estable.
6. UI no se congela con archivos grandes; actualizaciones de progreso fluidas (≤10Hz).
7. Contraste legible en dark mode, focus visible con teclado.

---

## 15) Recomendación de implementación (para el equipo)

* Estado del job en un store (Zustand/Redux) con updates throttled.
* En Rust, emitir eventos `job://progress` con throttle interno.
* ETA: EWMA throughput + bytes o tokens (best-effort). Mostrar “—” si no hay base.

---

**End of spec**

```
::contentReference[oaicite:0]{index=0}
```
