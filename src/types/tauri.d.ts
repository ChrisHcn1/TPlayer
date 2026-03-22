declare global {
  interface Window {
    __TAURI__?: {
      event: {
        listen: <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>
      }
    }
  }
}

export {}
