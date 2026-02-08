# Resumen Ejecutivo Final

## Documentos relacionados
- `README.md`
- `docs/00_INDICE_DOCUMENTACION_FINAL.md`
- `docs/02_ESPECIFICACION_MOTOR_FINAL.md`
- `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`
- `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`
- `docs/05_DECISIONES_PENDIENTES.md`

## 1) Vision del producto
Construir una app de escritorio para unir, depurar duplicados y exportar tokens desde multiples archivos de texto, con enfoque en:
- alto rendimiento con datasets grandes,
- comportamiento predecible,
- UI clara para usuarios tecnicos.

## 2) Decisiones funcionales cerradas
1. Dedupe case-sensitive.
   `Perro`, `perro` y `PERRO` se consideran tokens distintos.
2. Soporte Unicode completo.
   Incluye acentos, caracteres especiales y emojis.
3. Parsing de entrada robusto para fuentes no controladas.
   Delimitadores de entrada: whitespace, coma y punto y coma.
4. Salida por separador arbitrario (string).
   La salida respeta exactamente el separador elegido por el usuario.
5. Sin separador extra al final.
6. Modo DISK y tipo de orden deben ser seleccionables en UI.
7. Separador por defecto de producto: `"\n"`.
8. `Mode=Auto` en V1: alias de `Ram` (con tooltip explicito en UI).
9. Localizacion V1: ingles (`en`) y chino simplificado (`zh-CN`), sin hardcode de textos.

## 3) Comportamiento de salida consolidado
- El motor genera una secuencia de tokens unicos unidos por el separador final.
- Si el separador contiene salto de linea, habra multiples lineas.
- Si no contiene salto de linea, la salida normalmente queda en una linea logica.
- No se insertan saltos adicionales fuera del separador definido.

## 4) Estrategia de producto final
- Motor en Rust como nucleo reusable.
- UI desktop con Tauri v2 en una sola pantalla.
- Telemetria agregada en tiempo real, sin logs por token.
- ETA aproximada y util, sin sacrificar rendimiento.

## 5) Matriz de modos y orden (decision final)
- Ordering:
  - PreserveFirstSeen (default).
  - Alphabetical.
  - UnorderedFast.
- Mode:
  - Ram.
  - Disk.
  - Auto (en V1 funciona como alias de Ram).
- En `Disk + Alphabetical`, submodo:
  - FastBucketLocal (default recomendado).
  - GlobalPerfect (mas preciso, mas lento).

## 6) Limite funcional explicitado
`PreserveFirstSeen` solo garantiza orden global estable en RAM. En DISK no se garantiza orden global de primera aparicion en esta version.

## 7) Defaults recomendados de producto
- `mode = Ram`
- `ordering = PreserveFirstSeen`
- `disk_buckets = 256`
- `disk_run_bytes = 256MB` (escalable a 512MB segun hardware)
- `trim = ON`
- `drop_empty = ON`
- `output_separator_default = "\n"`

## 8) Criterios de calidad final
- Correctitud del dedupe exacto.
- Fluidez de UI bajo carga.
- Seguridad por capacidades minimas en Tauri.
- Accesibilidad minima AA en contraste y navegacion por teclado.
- Modo de error y cancelacion claros para operacion real.
