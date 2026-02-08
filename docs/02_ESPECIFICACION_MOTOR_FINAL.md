# Especificacion del Motor Rust (Final, sin codigo)

## Documentos relacionados
- `README.md`
- `docs/00_INDICE_DOCUMENTACION_FINAL.md`
- `docs/01_RESUMEN_EJECUTIVO_FINAL.md`
- `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`
- `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`
- `docs/05_DECISIONES_PENDIENTES.md`

## 1) Objetivo
Procesar uno o multiples archivos de texto para extraer tokens, eliminar duplicados y exportar el resultado en un archivo unico con separador configurable, manteniendo rendimiento alto en volumen grande.

## 2) Contrato funcional

### 2.1 Entrada
- Tipo: archivos de texto plano (por ejemplo `.txt`, `.csv` simple y formatos equivalentes en texto).
- Multiples archivos de entrada soportados.

### 2.2 Reglas de tokenizacion
- Delimitadores de entrada activos por defecto:
  - whitespace (espacios, tabs, saltos de linea),
  - coma `,`,
  - punto y coma `;`.
- El motor toma solo tokens.

### 2.3 Normalizacion
- `trim` por token: ON por defecto.
- descarte de token vacio: ON por defecto.
- No se aplica lowercase ni folding de case.

### 2.4 Dedupe
- Exacto, case-sensitive y Unicode-safe.
- Garantia: no aproximaciones ni tecnicas probabilisticas.

### 2.5 Salida
- Tokens unicos unidos por `output_separator` como string arbitrario.
- Soporta separadores simples y compuestos (ej.: `","`, `", "`, `",\n"`, `"\n"`, `";\n"`, `"\f"`).
- No agrega separador final.
- No agrega saltos extra fuera del separador definido.

## 3) Modos operativos

### 3.1 Mode
- `Ram`: cuando el dataset cabe en memoria.
- `Disk`: para volumen grande con uso de almacenamiento temporal.
- `Auto`: en V1 funciona como alias de `Ram` (la heuristica automatica real queda para version posterior).

### 3.2 Ordering
- `PreserveFirstSeen`: mantiene primer orden de aparicion (estable en RAM).
- `Alphabetical`: orden lexicografico determinista por bytes UTF-8.
- `UnorderedFast`: maxima velocidad sin garantia de orden.

### 3.3 Submodo para `Disk + Alphabetical`
- `FastBucketLocal`:
  - recomendado por defecto,
  - muy rapido,
  - no garantiza A-Z global perfecto.
- `GlobalPerfect`:
  - external merge sort,
  - garantiza A-Z global perfecto,
  - mayor costo de I/O y CPU.

## 4) Matriz de garantias final

### 4.1 RAM
| Ordering | Dedupe exacto | Orden de salida | Rendimiento |
| --- | --- | --- | --- |
| PreserveFirstSeen | Si | Estable global por primera aparicion | Muy alto |
| Alphabetical | Si | A-Z global perfecto | Alto |
| UnorderedFast | Si | No garantizado | Maximo |

### 4.2 DISK
| Ordering | Variante | Dedupe exacto | Orden de salida | Rendimiento |
| --- | --- | --- | --- | --- |
| UnorderedFast | N/A | Si | No garantizado | Muy alto |
| Alphabetical | FastBucketLocal | Si | Orden por bucket, no global perfecto | Muy alto |
| Alphabetical | GlobalPerfect | Si | A-Z global perfecto | Medio/alto |
| PreserveFirstSeen | N/A | Si | No garantizado globalmente en DISK | Alto |

## 5) Arquitectura final (alto nivel)
- Repositorio en workspace.
- `crates/core` como motor reusable y testeable.
- Integraciones esperadas:
  - UI desktop Tauri,
  - posible CLI,
  - benchmarks y pruebas.

### 5.1 Responsabilidades modulares (sin implementacion)
- Config: contrato de ejecucion.
- Tokenizacion: parser streaming con delimitadores definidos.
- Dedupe RAM: ejecucion rapida con orden segun modo.
- Dedupe DISK: buckets y/o orden global por merge sort externo.
- Writer: salida streaming y separador final.
- Progress/Stats: telemetria agregada para UI.
- Engine: orquestacion integral del job.

## 6) Principios de rendimiento confirmados
- Streaming en lectura y escritura.
- Reducir allocations en rutas calientes.
- Dedupe con estructuras hash de alto rendimiento.
- Separar trabajo pesado del frontend.
- Evitar computo o eventos de granularidad por token hacia UI.

## 7) Observabilidad requerida
- Progreso global best-effort.
- Etapa actual.
- Contadores operativos:
  - tokens vistos,
  - unicos,
  - duplicados,
  - throughput,
  - elapsed,
  - ETA aproximada cuando sea confiable.

## 8) Limites y notas de correctitud
1. Orden alfabetico es determinista por bytes UTF-8, no collation por locale humano.
2. `trim` Unicode-aware prioriza correctitud.
3. `PreserveFirstSeen` global en DISK no esta garantizado en esta version.
4. En `GlobalPerfect`, rendimiento depende de disco y tamano de runs.

## 9) Defaults operativos recomendados
- `mode = Ram`
- `mode_auto_behavior_v1 = RamAlias`
- `ordering = PreserveFirstSeen`
- `disk_alphabetical_mode = FastBucketLocal` cuando aplica
- `disk_buckets = 256`
- `disk_run_bytes = 256MB` (ajustable a 512MB segun hardware)
- `trim = true`
- `drop_empty = true`
- `output_separator_default = "\n"`

## 10) Evolucion prevista (sin romper V1)
1. Auto mode con heuristica real por muestreo.
2. Optimizacion de merge para menos allocs.
3. Multi-pass merge para casos extremos de runs.
4. Refinamiento de estimacion ETA en modos DISK grandes.
