# ADR-007: updater oficial Tauri con lease en el host

| Campo    | Valor                                                                 |
| -------- | --------------------------------------------------------------------- |
| Estado   | Aceptado                                                              |
| Fecha    | 2026-09-30                                                            |
| Alcance  | AppImage Linux, sin guest SDK updater/process                         |
| Decisión | `UpdateLease` host-owned + `tauri-plugin-updater` como único verificador |

## 1. Contexto

El launcher necesita actualizaciones firmadas del AppImage sin ampliar CSP, sin
confiar argumentos del WebView y sin romper el contrato multi-cliente (Wine /
`ro-sessiond` vivos).

## 2. Decisión

1. **Lease en el host.** `UpdateLease` es la máquina de estados. El WebView solo ve
   `UpdateSnapshot` y cuatro comandos sin argumentos de confianza.
2. **Plugin oficial como único verificador.** minisign vía `tauri-plugin-updater`.
   SHA-256 del release es evidencia humana, no autenticidad.
   `requireSignedVersion` impide emparejar un `version` inflado con un artefacto antiguo.
   El CLI de empaquetado debe ser 2.12 o posterior para que la firma lleve `version:`.
3. **Guest API denegada.** Se registra el plugin Rust para el backend. Las
   capabilities llevan `updater:deny-*` explícitos y CI falla si aparece
   `updater:default` / `updater:allow-*` o los paquetes JS `@tauri-apps/plugin-updater`
   / `@tauri-apps/plugin-process`.
4. **IdleClients.** Se atestigua antes de descargar y otra vez antes de reemplazar
   el AppImage, y otra vez al reiniciar. No se llama a `stop_all_games`.
5. **Reinicio explícito.** Tras instalar, fase `readyToRestart`. El usuario reinicia;
   no hay auto-restart.

## 3. Consecuencias

- El arranque no espera a la red: `useUpdateBootstrap` corre después de mostrar la
  ventana, fuera de `useAppInit`.
- Un cambio de pubkey deja fuera a clientes viejos hasta que reinstalan o reciben
  un puente firmado con la clave anterior.
- Desarrollo sin AppImage permanece en `unavailable`; no muta `current_exe`.
