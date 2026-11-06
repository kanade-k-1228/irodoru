import { useInferenceStatus } from "@/logic/hooks/useInferenceStatus.ts";
import { useLibrary } from "@/logic/hooks/useLibrary.ts";
import { AudioEditor } from "@/view/editor/AudioEditor.tsx";
import { Header } from "@/view/header/Header.tsx";
import { Composer } from "@/view/timeline/Composer.tsx";

export const App = () => {
  const save = useLibrary();
  useInferenceStatus();
  return (
    <div className="studio">
      <Header onSave={save} />
      <Composer />
      <AudioEditor />
    </div>
  );
};
