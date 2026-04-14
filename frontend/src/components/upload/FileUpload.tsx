import { useState, useCallback, useEffect, useRef } from 'react';
import { useDropzone } from 'react-dropzone';
import { useApi } from '@/hooks/useApi';
import { useHealthStore } from '@/stores/healthStore';
import { createUploadApi } from '@/api/uploads';
import { ColumnMappingModal } from './ColumnMappingModal';
import type { Upload, UploadPreview, FileFormat } from '@/types/models';

const ACCEPTED_EXTENSIONS: Record<string, FileFormat> = {
  '.csv': 'csv',
  '.tsv': 'tsv',
  '.ofx': 'ofx',
  '.qfx': 'qfx',
  '.qbo': 'qbo',
  '.qif': 'qif',
  '.xls': 'xls',
  '.xlsx': 'xlsx',
};

const ACCEPT_MAP: Record<string, string[]> = {
  'text/csv': ['.csv'],
  'text/tab-separated-values': ['.tsv'],
  'application/x-ofx': ['.ofx'],
  'application/x-qfx': ['.qfx'],
  'application/vnd.intu.qbo': ['.qbo'],
  'application/qif': ['.qif'],
  'application/vnd.ms-excel': ['.xls'],
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet': ['.xlsx'],
};

function detectFormat(fileName: string): FileFormat | null {
  const ext = fileName.slice(fileName.lastIndexOf('.')).toLowerCase();
  return ACCEPTED_EXTENSIONS[ext] ?? null;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface FileUploadProps {
  accountId: string;
  onImportComplete?: () => void;
}

export function FileUpload({ accountId, onImportComplete }: FileUploadProps) {
  const api = useApi();
  const uploadApi = createUploadApi(api);
  const llmStatus = useHealthStore((s) => s.llmStatus);
  const llmReady = llmStatus === 'ready';

  const [file, setFile] = useState<File | null>(null);
  const [detectedFormat, setDetectedFormat] = useState<FileFormat | null>(null);
  const [uploading, setUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState(0);
  const [upload, setUpload] = useState<Upload | null>(null);
  const [preview, setPreview] = useState<UploadPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [processingStatus, setProcessingStatus] = useState<Upload | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const onDrop = useCallback((accepted: File[]) => {
    const dropped = accepted[0];
    if (!dropped) return;
    const format = detectFormat(dropped.name);
    if (!format) {
      setError('Unsupported file format');
      return;
    }
    setFile(dropped);
    setDetectedFormat(format);
    setError(null);
    setUpload(null);
    setPreview(null);
  }, []);

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    onDrop,
    accept: ACCEPT_MAP,
    maxFiles: 1,
    multiple: false,
    disabled: !llmReady,
  });

  const handleUpload = async () => {
    if (!file) return;
    setUploading(true);
    setError(null);
    try {
      const result = await uploadApi.uploadFile(accountId, file, setUploadProgress);
      setUpload(result);
      const previewData = await uploadApi.getPreview(result.id);
      setPreview(previewData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Upload failed');
    } finally {
      setUploading(false);
    }
  };

  useEffect(() => {
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  const handleMappingComplete = () => {
    const currentUpload = upload;
    setPreview(null);

    if (currentUpload) {
      setProcessingStatus(currentUpload);
      pollRef.current = setInterval(async () => {
        try {
          const status = await uploadApi.getUploadStatus(currentUpload.id);
          setProcessingStatus(status);
          if (status.status === 'complete' || status.status === 'error') {
            if (pollRef.current) clearInterval(pollRef.current);
            pollRef.current = null;
            if (status.status === 'complete') {
              setFile(null);
              setUpload(null);
              setProcessingStatus(null);
              setDetectedFormat(null);
              onImportComplete?.();
            } else {
              setError(status.error_message ?? 'Import failed');
              setProcessingStatus(null);
            }
          }
        } catch {
          if (pollRef.current) clearInterval(pollRef.current);
          pollRef.current = null;
          setFile(null);
          setUpload(null);
          setProcessingStatus(null);
          setDetectedFormat(null);
          onImportComplete?.();
        }
      }, 2000);
    } else {
      setFile(null);
      setUpload(null);
      setDetectedFormat(null);
      onImportComplete?.();
    }
  };

  if (processingStatus) {
    const statusLabel =
      processingStatus.status === 'importing'
        ? 'Importing transactions...'
        : processingStatus.status === 'categorizing'
          ? 'Categorizing transactions...'
          : `Processing (${processingStatus.status})...`;
    return (
      <div className="space-y-3" aria-live="polite">
        <div className="status-banner">
          <div className="w-4 h-4 border-2 border-[var(--color-primary)] border-t-transparent rounded-full animate-spin" />
          <div>
            <p className="text-sm font-medium text-[var(--color-primary)]">{statusLabel}</p>
            {processingStatus.imported_count !== null && (
              <p className="text-xs text-[var(--color-primary)]">
                {processingStatus.imported_count} imported
                {processingStatus.skipped_count
                  ? `, ${processingStatus.skipped_count} skipped`
                  : ''}
              </p>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (preview && upload) {
    return (
      <ColumnMappingModal
        preview={preview}
        uploadId={upload.id}
        onComplete={handleMappingComplete}
        onCancel={() => {
          setPreview(null);
          setUpload(null);
        }}
      />
    );
  }

  return (
    <div className="space-y-4">
      {!llmReady && (
        <div
          className="flex items-center gap-2 p-3 bg-amber-50 border border-amber-200 rounded-lg text-sm text-amber-800"
          role="status"
        >
          <span
            className="inline-block w-2 h-2 rounded-full bg-amber-400 animate-pulse"
            aria-hidden="true"
          />
          {llmStatus === 'failed'
            ? 'AI model failed to load — uploads that require categorization are unavailable.'
            : 'AI model is loading — upload will be available once it is ready.'}
        </div>
      )}

      <div
        {...getRootProps()}
        role="button"
        tabIndex={llmReady ? 0 : -1}
        aria-disabled={!llmReady}
        aria-label="File drop zone. Drop a file or press Enter to browse. Supported formats: CSV, TSV, OFX, QFX, QBO, QIF, XLS, XLSX."
        className={`border-2 border-dashed rounded-lg p-8 text-center transition-colors ${
          !llmReady
            ? 'border-[var(--color-border)] bg-[var(--color-surface)] cursor-not-allowed opacity-60'
            : isDragActive
              ? 'border-[var(--color-primary)] bg-[var(--color-primary-subtle)] cursor-pointer'
              : 'border-[var(--color-input-border)] hover:border-[var(--color-text-secondary)] cursor-pointer'
        }`}
      >
        <input {...getInputProps()} aria-label="Choose file to upload" />
        <div className="text-4xl mb-2" aria-hidden="true">
          📁
        </div>
        <p className="text-sm font-medium text-[var(--color-text)]">
          {isDragActive ? 'Drop the file here' : 'Drop file or click to browse'}
        </p>
        <p className="text-xs text-[var(--color-text-secondary)] mt-1">
          CSV, TSV, OFX, QFX, QBO, QIF, XLS, XLSX
        </p>
      </div>

      {error && (
        <div
          className="p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700"
          role="alert"
          aria-live="assertive"
        >
          {error}
        </div>
      )}

      {file && !uploading && !upload && (
        <div className="flex items-center justify-between p-3 bg-[var(--color-surface)] border border-[var(--color-border)] rounded-lg">
          <div>
            <p className="text-sm font-medium text-[var(--color-text)]">{file.name}</p>
            <p className="text-xs text-[var(--color-text-secondary)]">
              {formatFileSize(file.size)}
              {detectedFormat && (
                <span className="ml-2 inline-block px-1.5 py-0.5 badge-primary rounded text-xs font-medium uppercase">
                  {detectedFormat}
                </span>
              )}
            </p>
          </div>
          <button
            onClick={handleUpload}
            disabled={!llmReady}
            className={`px-4 py-2 text-sm font-medium rounded-lg transition-colors ${
              llmReady
                ? 'btn-primary'
                : 'bg-[var(--color-primary-muted)] cursor-not-allowed text-white'
            }`}
          >
            Upload
          </button>
        </div>
      )}

      {uploading && (
        <div className="space-y-2" aria-live="polite">
          <div className="flex items-center justify-between text-sm text-[var(--color-text-secondary)]">
            <span>Uploading...</span>
            <span>{uploadProgress}%</span>
          </div>
          <div
            className="w-full bg-[var(--color-border)] rounded-full h-2"
            role="progressbar"
            aria-valuenow={uploadProgress}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`Upload progress: ${uploadProgress}%`}
          >
            <div
              className="progress-fill transition-all duration-300"
              style={{ width: `${uploadProgress}%` }}
            />
          </div>
        </div>
      )}
    </div>
  );
}
