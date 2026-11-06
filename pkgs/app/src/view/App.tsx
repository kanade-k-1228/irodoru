import { useInferenceStatus } from "@/logic/hooks/useInferenceStatus.ts";
import { useLibrary } from "@/logic/hooks/useLibrary.ts";
import { useTheme } from "@/logic/hooks/useTheme.ts";
import { AudioEditor } from "@/view/editor/AudioEditor.tsx";
import { Header } from "@/view/header/Header.tsx";
import { Composer } from "@/view/timeline/Composer.tsx";

export const App = () => {
  const project = useLibrary();
  useTheme();
  useInferenceStatus();
  return (
    <div className="studio">
      <Header project={project} />
      <Composer />
      <AudioEditor />
      <footer className="studio-footer">
        <span className="sr-only">プロジェクトの保存先: </span>
        <span className="block truncate" title={project.status.path ?? undefined}>
          {project.status.path ?? "プロジェクト未選択"}
        </span>
      </footer>
    </div>
  );
};
