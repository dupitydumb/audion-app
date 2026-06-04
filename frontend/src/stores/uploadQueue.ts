import { get, writable } from 'svelte/store';

export interface UploadItem {
  id: number;
  file: File;
  progress: number;
  status: 'pending' | 'uploading' | 'success' | 'error' | 'duplicate';
  errorMsg?: string;
}

type ToastType = 'success' | 'error' | 'info';

export const uploadQueue = writable<UploadItem[]>([]);
export const isUploading = writable(false);

let nextId = 0;
let authToken = '';
let notify: ((message: string, type: ToastType) => void) | null = null;

export function configureUploadQueue(params: {
  token: string;
  addToast: (message: string, type: ToastType) => void;
}) {
  authToken = params.token;
  notify = params.addToast;
}

export function addFilesToQueue(fileList: FileList) {
  const validExtensions = ['.mp3', '.flac', '.ogg', '.wav', '.m4a'];
  const newItems: UploadItem[] = [];

  for (let i = 0; i < fileList.length; i++) {
    const file = fileList[i];
    const ext = '.' + file.name.split('.').pop()?.toLowerCase();

    if (!validExtensions.includes(ext)) {
      notify?.(`Unsupported file type: ${file.name}`, 'error');
      continue;
    }

    newItems.push({
      id: nextId++,
      file,
      progress: 0,
      status: 'pending'
    });
  }

  if (newItems.length === 0) return;
  uploadQueue.update(items => [...items, ...newItems]);
  processQueue();
}

export function clearCompleted() {
  uploadQueue.update(items =>
    items.filter(item => item.status === 'pending' || item.status === 'uploading')
  );
}

export function summarizeQueue(items: UploadItem[]) {
  const summary = {
    total: items.length,
    pending: 0,
    uploading: 0,
    success: 0,
    error: 0,
    duplicate: 0
  };

  for (const item of items) {
    if (item.status === 'pending') summary.pending += 1;
    else if (item.status === 'uploading') summary.uploading += 1;
    else if (item.status === 'success') summary.success += 1;
    else if (item.status === 'error') summary.error += 1;
    else if (item.status === 'duplicate') summary.duplicate += 1;
  }

  return summary;
}

function updateItem(id: number, patch: Partial<UploadItem>) {
  uploadQueue.update(items =>
    items.map(item => (item.id === id ? { ...item, ...patch } : item))
  );
}

function processQueue() {
  if (get(isUploading)) return;

  const pendingItem = get(uploadQueue).find(item => item.status === 'pending');
  if (!pendingItem) {
    isUploading.set(false);
    return;
  }

  isUploading.set(true);
  updateItem(pendingItem.id, { status: 'uploading' });

  uploadFile(pendingItem)
    .catch(() => {
      // uploadFile handles per-item errors
    })
    .finally(() => {
      isUploading.set(false);
      processQueue();
    });
}

function uploadFile(item: UploadItem): Promise<void> {
  return new Promise((resolve) => {
    if (!authToken) {
      updateItem(item.id, { status: 'error', errorMsg: 'Missing auth token' });
      notify?.(`Failed: ${item.file.name}`, 'error');
      resolve();
      return;
    }

    const formData = new FormData();
    formData.append('file', item.file);

    const xhr = new XMLHttpRequest();
    xhr.open('POST', '/api/tracks');
    xhr.setRequestHeader('Authorization', `Bearer ${authToken}`);

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable) {
        const percent = Math.round((e.loaded / e.total) * 100);
        updateItem(item.id, { progress: percent });
      }
    });

    xhr.onload = () => {
      if (xhr.status === 201) {
        updateItem(item.id, { status: 'success', progress: 100 });
        notify?.(`Uploaded: ${item.file.name}`, 'success');
      } else if (xhr.status === 409) {
        updateItem(item.id, {
          status: 'duplicate',
          progress: 100,
          errorMsg: 'Duplicate track'
        });
        notify?.(`Duplicate skipped: ${item.file.name}`, 'info');
      } else {
        updateItem(item.id, {
          status: 'error',
          errorMsg: xhr.responseText || `Error ${xhr.status}`
        });
        notify?.(`Failed: ${item.file.name}`, 'error');
      }
      resolve();
    };

    xhr.onerror = () => {
      updateItem(item.id, { status: 'error', errorMsg: 'Network error' });
      notify?.(`Network error: ${item.file.name}`, 'error');
      resolve();
    };

    xhr.send(formData);
  });
}
