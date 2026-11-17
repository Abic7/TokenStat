import {Composition} from 'remotion';
import {Promo} from './Promo';
export const Root = () => (
  <Composition id="Promo" component={Promo} durationInFrames={580} fps={30} width={1280} height={640} />
);
