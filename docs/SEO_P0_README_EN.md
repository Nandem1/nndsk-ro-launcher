# SEO P0 — sección inglesa en `README.md`

| Campo | Valor |
| --- | --- |
| Estado | Plan cerrado. Composer implementa; este documento no edita `README.md`. |
| Archivo a cambiar | [`README.md`](../README.md) (raíz del repo, no `docs/README.md`) |
| Base verificada | `main` @ `21073df` (`Merge pull request #21`) |
| Fuera de este plan | Fast-forward, merge, AppImage/Flatpak, homepage de GitHub, `docs/README.md` |

## 1. Resultado

Insertar **una** sección H2 en inglés, justo debajo de la intro española y **antes** de
`## Funciones principales`. El resto del README (incluye Uso, Desarrollo, supervisor y matriz
Gepard) no se toca.

## 2. Estructura actual en `main` (verificada)

El fichero raíz empieza así (líneas 1–8):

```markdown
# RO-Launcher

Launcher de Ragnarok Online para Linux, construido con Tauri, React y Rust. Administra runners,
WINEPREFIX, dependencias y herramientas por servidor sin depender de la versión de Wine instalada
por el sistema.

## Funciones principales
```

No hay otra H2 entre la intro y `Funciones principales`. Esa línea en blanco (hoy línea 6) es el
hueco de inserción.

Anclas GitHub que el bloque usa y que ya existen:

- `## Desarrollo` → `#desarrollo`

## 3. Evidencia de claims (no inferencia)

| Claim del bloque EN | Evidencia | Decisión |
| --- | --- | --- |
| Launcher de escritorio para RO en Linux | Intro de `README.md`; página de producto | Incluir |
| Runners Wine/Proton por servidor | `README.md` Funciones / Matriz Gepard; `AGENTS.md` | Incluir |
| WINEPREFIX aislado | Intro + Datos locales | Incluir |
| DXVK → Vulkan; dgVoodoo opcional | Pipeline gráfico del README | Incluir |
| AutoPot/AutoBuff sin `ptrace_scope=0` | README «Supervisor de sesión y memoria»; `docs/PTRACE_SESSION_SUPERVISOR_PLAN.md` | Incluir |
| Proton-CachyOS + UMU administrados, o Wine portable | README Uso / Wine 7.16 portable | Incluir |
| Enlace Releases aunque esté vacío | `gh api .../releases` → `0` releases el 2026-09-27 | Incluir el enlace; **no** decir que hay binario |
| AppImage / Flatpak publicados | Cero releases; no hay Flathub; `npm run tauri:build:appimage` es build de desarrollo | **Prohibido** afirmar descarga o paquete publicado |
| Etiqueta del enlace de producto | Vicente: «Product page», no «Case study». URL viva: https://nndsk.dev/projects/nndsk-ro-launcher/ | Usar exactamente `[Product page](...)` |
| Botón de la UI | `LaunchButton.tsx`: etiqueta **Jugar** (icono Lucide `Play`) | El copy EN usa **Play** (SEO); no inventar un botón inglés en la app |

## 4. Bloque markdown exacto a insertar

Pegar **verbatim** entre la intro española y `## Funciones principales`. Una línea en blanco
antes del H2 nuevo y una línea en blanco después del último ítem, igual que el resto del README.

```markdown
## Play Ragnarok Online on Linux

**nndsk-ro-launcher (RO-Launcher)** is a desktop launcher for **Ragnarok Online on Linux**. It
manages per-server Wine/Proton runners, isolated WINEPREFIX, DXVK → Vulkan, optional dgVoodoo, plus
AutoPot/AutoBuff without requiring `ptrace_scope=0`.

1. Add your client `.exe` and pick a server profile.
2. Prepare the environment (managed Proton-CachyOS + UMU, or portable Wine).
3. Hit **Play**.

- [Product page](https://nndsk.dev/projects/nndsk-ro-launcher/)
- [Releases](https://github.com/Nandem1/nndsk-ro-launcher/releases)
- Build from source: see [**Desarrollo**](#desarrollo) below
```

## 5. Resultado esperado del encabezado (después del insert)

```markdown
# RO-Launcher

Launcher de Ragnarok Online para Linux, construido con Tauri, React y Rust. Administra runners,
WINEPREFIX, dependencias y herramientas por servidor sin depender de la versión de Wine instalada
por el sistema.

## Play Ragnarok Online on Linux

**nndsk-ro-launcher (RO-Launcher)** is a desktop launcher for **Ragnarok Online on Linux**. It
manages per-server Wine/Proton runners, isolated WINEPREFIX, DXVK → Vulkan, optional dgVoodoo, plus
AutoPot/AutoBuff without requiring `ptrace_scope=0`.

1. Add your client `.exe` and pick a server profile.
2. Prepare the environment (managed Proton-CachyOS + UMU, or portable Wine).
3. Hit **Play**.

- [Product page](https://nndsk.dev/projects/nndsk-ro-launcher/)
- [Releases](https://github.com/Nandem1/nndsk-ro-launcher/releases)
- Build from source: see [**Desarrollo**](#desarrollo) below

## Funciones principales
```

## 6. Prohibido en la implementación

- Reemplazar, traducir o mover la intro española.
- Insertar la sección EN debajo de `Funciones principales`, en Uso, o al final del README.
- Etiqueta «Case study» (u otra) para https://nndsk.dev/projects/nndsk-ro-launcher/
- Afirmar AppImage, Flatpak, Flathub, `.deb`, binario en Releases, o «download the latest release».
- Afirmar compatibilidad universal con todos los servidores privados.
- Mencionar bypass, hook o modificación de Gepard/GameGuard.
- Pedir `ptrace_scope=0` o `sudo`.
- Editar `docs/README.md` (índice de arquitectura) ni este plan, salvo marcar estado si se pide.
- Fast-forward ni merge. PR draft hasta revisión humana.

## 7. Pasos de Composer (únicos)

1. Abrir `README.md` en la raíz.
2. Sustituir el hueco entre la intro española y `## Funciones principales` por el bloque de la
   sección 4 (más las líneas en blanco descritas).
3. `git diff --check` y comprobar a ojo el encabezado contra la sección 5.
4. Confirmar que `](#desarrollo)` apunta a `## Desarrollo` (línea 75 hoy; el número se desplazará
   ~15 líneas).
5. No correr gates de frontend/Rust: cambio de un bloque Markdown en README.

## 8. Validación de este plan (ya hecha)

- `git fetch origin main` → HEAD = `origin/main` = `21073df`.
- Intro ES + H2 `Funciones principales` leídos del README de `main`.
- GET https://nndsk.dev/projects/nndsk-ro-launcher/ → página de producto viva (no case study).
- GitHub Releases del repo: lista vacía; el enlace sigue siendo válido.
- UI: `src/features/launcher/LaunchButton.tsx` etiqueta **Jugar** / **Preparar entorno**.
- No hay publicación Flatpak/AppImage en el repo ni en Releases.

## 9. Fuera de alcance (no bloquear P0)

- `homepage` vacío en el repo GitHub (se podría apuntar a nndsk.dev más adelante).
- Traducir el resto del README.
- Publicar un release o un AppImage.
- Indexar este fichero en `docs/README.md`.
