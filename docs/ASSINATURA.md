# Assinar o Maré com o certificado open source da Certum

Sem assinatura digital, o Smart App Control do Windows bloqueia o `Mare.exe`
("Uma política de Controle de Aplicativo bloqueou este arquivo"). A Microsoft diz que o
Smart App Control deixa rodar apps assinados com certificado de uma autoridade confiável.
A Certum é uma delas e vende um certificado barato para projetos open source.

> A Microsoft não garante que todo app assinado passe: a checagem na nuvem continua valendo.
> Teste o `.exe` assinado num Windows com o Smart App Control ligado antes de divulgar.

## Antes de comprar

- **É para pessoa física.** O certificado sai no seu nome, com "Open Source Developer" no
  campo do titular.
- **O projeto precisa ser público e não comercial.** Se a Certum achar que o certificado
  assina software vendido, ela revoga.
- **A Certum precisa ver a sua ligação com o projeto** pelas informações públicas. O
  repositório fica na organização `neonio-it`, e o README diz que você mantém o Maré.
  Use no pedido o mesmo nome que está no README e no seu documento. Se a Certum não
  conseguir identificar o projeto, ela não emite.
- **Documentos:** documento de identidade e um comprovante de endereço em seu nome (conta
  de consumo). Confira a lista atual em
  [Code Signing – required documents](https://support.certum.eu/en/code-signing-required-documents/).
- **Validade:** 1 ano. Depois é preciso renovar.

## Qual versão comprar

Existem três versões do "Open Source Code Signing". Os preços variam entre as lojas da
Certum, então confira na hora:

| Versão | Preço aproximado | Como a chave fica guardada |
| --- | --- | --- |
| **Na nuvem (SimplySign)** | ~€49/ano | Nos servidores da Certum; você libera pelo app SimplySign no celular |
| Com cartão e leitor | ~€69/ano | Num cartão criptográfico que chega pelo correio |
| Só o código de ativação | ~€25/ano | Para quem já tem cartão e leitor da Certum |

**Recomendado: a versão na nuvem.** Não precisa de hardware, e o script deste projeto já
funciona com ela. A versão na nuvem tem limite de 5.000 assinaturas por mês, muito mais
do que o Maré precisa.

Desde 2023, nenhuma autoridade entrega mais a chave de assinatura de código como arquivo
`.pfx`. Ela sempre fica num cartão ou na nuvem, e por isso existe o SimplySign.

## Passo a passo

1. **Repositório público:** <https://github.com/neonio-it/mare-torrent>.
2. **Comprar** o "Open Source Code Signing in the Cloud" na
   [loja da Certum](https://shop.certum.eu/open-source-code-signing.html).
3. **Preencher o pedido** com o endereço do projeto (o link acima) e enviar os documentos.
4. **Esperar a validação.** A Certum confere a identidade e o projeto e avisa por e-mail.
5. **Ativar o SimplySign** seguindo o e-mail da Certum:
   - instale o app **SimplySign** no celular;
   - instale o **SimplySign Desktop** no PC.
6. **Conectar:** abra o SimplySign Desktop e entre com o código que aparece no app do
   celular. O certificado passa a aparecer no Windows em
   *Certificados – Usuário atual › Pessoal*.
7. **Assinar:** na pasta do projeto, rode

   ```powershell
   .\scripts\compilar-e-assinar.ps1
   ```

   O script compila, copia para `dist\Mare.exe`, assina com carimbo de data e hora da
   Certum e confere o resultado. No fim ele mostra o SHA-256 do arquivo.
8. **Publicar:** crie uma *Release* no GitHub e anexe o `dist\Mare.exe` assinado.

Toda versão nova precisa ser assinada de novo (passo 6 em diante).

## Se der errado

| Mensagem | O que fazer |
| --- | --- |
| `nenhum certificado da Certum disponível` | O SimplySign Desktop não está conectado. Conecte e rode de novo. |
| `signtool.exe não encontrado` | Instale o Windows SDK com o componente *Signing Tools*. |
| `a assinatura não foi validada pelo Windows` | Veja a mensagem que vem junto; normalmente é certificado vencido ou cadeia incompleta. |
| O Windows ainda bloqueia o `.exe` assinado | Confira se a assinatura aparece em *Propriedades › Assinaturas Digitais*. Se aparecer e o Smart App Control mesmo assim bloquear, é a checagem na nuvem: abra um chamado com a Certum ou com a Microsoft. |
