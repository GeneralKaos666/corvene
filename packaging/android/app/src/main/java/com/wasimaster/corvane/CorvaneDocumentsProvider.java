package com.wasimaster.corvane;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract.Document;
import android.provider.DocumentsContract.Root;
import android.provider.DocumentsProvider;
import android.webkit.MimeTypeMap;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/**
 * Shows Corvane's repositories (files/repositories in the app-private
 * storage) to other applications through the Storage Access Framework, so an
 * editor or a file manager can open and change the files of a clone.
 *
 * A document id is "repositories" for the folder itself and
 * "repositories/<path below it>" for everything in it.
 */
public class CorvaneDocumentsProvider extends DocumentsProvider {
    private static final String ROOT_ID = "repositories";
    /** The document id of the repositories folder. */
    static final String BASE_ID = "repositories";

    private static final String[] ROOT_COLUMNS = {
        Root.COLUMN_ROOT_ID, Root.COLUMN_MIME_TYPES, Root.COLUMN_FLAGS, Root.COLUMN_ICON,
        Root.COLUMN_TITLE, Root.COLUMN_SUMMARY, Root.COLUMN_DOCUMENT_ID,
        Root.COLUMN_AVAILABLE_BYTES,
    };

    private static final String[] DOCUMENT_COLUMNS = {
        Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE, Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_LAST_MODIFIED, Document.COLUMN_FLAGS, Document.COLUMN_SIZE,
    };

    /** files/repositories, created on first use. */
    static File baseDirectory(android.content.Context context) {
        File base = new File(context.getFilesDir(), "repositories");
        base.mkdirs();
        return base;
    }

    private File base() {
        return baseDirectory(getContext());
    }

    /** The file of a document id, refusing ids that leave the folder. */
    private File fileFor(String documentId) throws FileNotFoundException {
        File base = base();
        File file = fileOf(base, documentId);
        if (file == null) {
            throw new FileNotFoundException(documentId);
        }
        try {
            String canonical = file.getCanonicalPath();
            String root = base.getCanonicalPath();
            if (!canonical.equals(root) && !canonical.startsWith(root + File.separator)) {
                throw new FileNotFoundException(documentId);
            }
        } catch (IOException e) {
            throw new FileNotFoundException(documentId);
        }
        if (!file.exists()) {
            throw new FileNotFoundException(documentId);
        }
        return file;
    }

    /** The file a document id names below `base`; null for a foreign id. */
    static File fileOf(File base, String documentId) {
        if (BASE_ID.equals(documentId)) {
            return base;
        }
        if (documentId.startsWith(BASE_ID + "/")) {
            return new File(base, documentId.substring(BASE_ID.length() + 1));
        }
        return null;
    }

    private String idFor(File file) {
        String root = base().getAbsolutePath();
        String path = file.getAbsolutePath();
        return path.length() <= root.length()
                ? BASE_ID : BASE_ID + "/" + path.substring(root.length() + 1);
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Cursor queryRoots(String[] projection) {
        MatrixCursor result = new MatrixCursor(projection != null ? projection : ROOT_COLUMNS);
        File base = base();
        MatrixCursor.RowBuilder row = result.newRow();
        row.add(Root.COLUMN_ROOT_ID, ROOT_ID);
        row.add(Root.COLUMN_DOCUMENT_ID, BASE_ID);
        row.add(Root.COLUMN_TITLE, getContext().getString(R.string.app_name));
        row.add(Root.COLUMN_SUMMARY, getContext().getString(R.string.documents_summary));
        row.add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_CREATE | Root.FLAG_SUPPORTS_IS_CHILD
                | Root.FLAG_LOCAL_ONLY);
        row.add(Root.COLUMN_MIME_TYPES, "*/*");
        row.add(Root.COLUMN_AVAILABLE_BYTES, base.getFreeSpace());
        row.add(Root.COLUMN_ICON, R.mipmap.ic_launcher);
        return result;
    }

    @Override
    public Cursor queryDocument(String documentId, String[] projection)
            throws FileNotFoundException {
        MatrixCursor result = new MatrixCursor(projection != null ? projection : DOCUMENT_COLUMNS);
        addRow(result, fileFor(documentId));
        return result;
    }

    @Override
    public Cursor queryChildDocuments(String parentDocumentId, String[] projection,
            String sortOrder) throws FileNotFoundException {
        MatrixCursor result = new MatrixCursor(projection != null ? projection : DOCUMENT_COLUMNS);
        File[] children = fileFor(parentDocumentId).listFiles();
        if (children != null) {
            for (File child : children) {
                addRow(result, child);
            }
        }
        return result;
    }

    private void addRow(MatrixCursor result, File file) {
        int flags = 0;
        if (file.isDirectory()) {
            if (file.canWrite()) {
                flags |= Document.FLAG_DIR_SUPPORTS_CREATE;
            }
        } else if (file.canWrite()) {
            flags |= Document.FLAG_SUPPORTS_WRITE;
        }
        File parent = file.getParentFile();
        if (parent != null && parent.canWrite() && !file.equals(base())) {
            flags |= Document.FLAG_SUPPORTS_DELETE | Document.FLAG_SUPPORTS_RENAME;
        }
        MatrixCursor.RowBuilder row = result.newRow();
        row.add(Document.COLUMN_DOCUMENT_ID, idFor(file));
        row.add(Document.COLUMN_DISPLAY_NAME,
                file.equals(base()) ? getContext().getString(R.string.app_name) : file.getName());
        row.add(Document.COLUMN_SIZE, file.length());
        row.add(Document.COLUMN_MIME_TYPE, mimeType(file));
        row.add(Document.COLUMN_LAST_MODIFIED, file.lastModified());
        row.add(Document.COLUMN_FLAGS, flags);
    }

    private static String mimeType(File file) {
        if (file.isDirectory()) {
            return Document.MIME_TYPE_DIR;
        }
        String name = file.getName();
        int dot = name.lastIndexOf('.');
        if (dot >= 0) {
            String type = MimeTypeMap.getSingleton()
                    .getMimeTypeFromExtension(name.substring(dot + 1).toLowerCase());
            if (type != null) {
                return type;
            }
        }
        return "application/octet-stream";
    }

    @Override
    public String getDocumentType(String documentId) throws FileNotFoundException {
        return mimeType(fileFor(documentId));
    }

    @Override
    public boolean isChildDocument(String parentDocumentId, String documentId) {
        return documentId.equals(parentDocumentId)
                || documentId.startsWith(parentDocumentId + "/");
    }

    @Override
    public ParcelFileDescriptor openDocument(String documentId, String mode,
            CancellationSignal signal) throws FileNotFoundException {
        return ParcelFileDescriptor.open(fileFor(documentId),
                ParcelFileDescriptor.parseMode(mode));
    }

    @Override
    public String createDocument(String parentDocumentId, String mimeType, String displayName)
            throws FileNotFoundException {
        File parent = fileFor(parentDocumentId);
        if (displayName.contains("/") || displayName.equals("..") || displayName.equals(".")) {
            throw new FileNotFoundException(displayName);
        }
        File file = new File(parent, displayName);
        try {
            boolean created = Document.MIME_TYPE_DIR.equals(mimeType)
                    ? file.mkdir() : file.createNewFile();
            if (!created) {
                throw new FileNotFoundException("could not create " + displayName);
            }
        } catch (IOException e) {
            throw new FileNotFoundException(e.getMessage());
        }
        return idFor(file);
    }

    @Override
    public void deleteDocument(String documentId) throws FileNotFoundException {
        if (BASE_ID.equals(documentId) || !deleteRecursively(fileFor(documentId))) {
            throw new FileNotFoundException("could not delete " + documentId);
        }
    }

    private static boolean deleteRecursively(File file) {
        File[] children = file.isDirectory() ? file.listFiles() : null;
        if (children != null) {
            for (File child : children) {
                deleteRecursively(child);
            }
        }
        return file.delete();
    }

    @Override
    public String renameDocument(String documentId, String displayName)
            throws FileNotFoundException {
        File file = fileFor(documentId);
        if (BASE_ID.equals(documentId) || displayName.contains("/")
                || displayName.equals("..")) {
            throw new FileNotFoundException(displayName);
        }
        File renamed = new File(file.getParentFile(), displayName);
        if (renamed.exists() || !file.renameTo(renamed)) {
            throw new FileNotFoundException("could not rename " + documentId);
        }
        return idFor(renamed);
    }
}
