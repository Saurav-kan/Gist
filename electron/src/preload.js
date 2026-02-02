const { contextBridge, ipcRenderer } = require('electron');

// Expose protected methods that allow the renderer process to use
// the ipcRenderer without exposing the entire object
contextBridge.exposeInMainWorld('electronAPI', {
  apiRequest: (method, endpoint, data) =>
    ipcRenderer.invoke('api-request', { method, endpoint, data }),
  selectDirectory: () =>
    ipcRenderer.invoke('select-directory'),
  openFile: (filePath) =>
    ipcRenderer.invoke('open-file', filePath),
  showInFolder: (filePath) =>
    ipcRenderer.invoke('show-in-folder', filePath),
  readFilePreview: (filePath) =>
    ipcRenderer.invoke('read-file-preview', filePath),
  windowMinimize: () =>
    ipcRenderer.invoke('window-minimize'),
  windowMaximize: () =>
    ipcRenderer.invoke('window-maximize'),
  windowClose: () =>
    ipcRenderer.invoke('window-close')
});
