<?php
require '/source/Common.php';
$queries = [
'idsite[]=1&rec=1', 'idsite[site]=1&rec=1', 'idsite=1&rec[]=1',
'idsite=0&idsite[]=1&idsite=2&rec=1', 'idsite=1&idsite[]=2&rec=1',
'idsite=1&rec=1&e_c[]=Video&e_a=play', 'idsite=1&rec=1&e_c=Video&e_a[]=play',
'idsite=1&rec=1&ec_items[0][0]=sku&ec_items[0][1]=Name&ec_items[0][2][0]=One&ec_items[0][2][1]=Two&ec_items[0][3]=1&ec_items[0][4]=2',
'idsite=1&rec=1&uadata[brands][0][brand]=Chromium&uadata[brands][0][version]=126&uadata[mobile]=0&uadata[platform]=Linux',
'idsite=1&rec=1&uadata[0]=a&uadata[]=b',
'idsite=1&rec=1&uadata[2]=a&uadata[]=b',
'idsite=1&rec=1&uadata[01]=a&uadata[1]=b&uadata[-0]=c',
'idsite=1&rec=1&uadata[a]=scalar&uadata[a][x]=nested',
'idsite=1&rec=1&uadata[a][x]=nested&uadata[a]=scalar',
'idsite=1&rec=1&uadata[][x]=a&uadata[][x]=b',
'idsite=1&rec=1&uadata[x]suffix=a&uadata[x][y]suffix=b',
'idsite=1&rec=1&uadata[a.b]=dot&uadata[a+b]=space',
'idsite=1&rec=1&uadata[ ]=blank&uadata[%09]=tab&uadata[  ]=twospace',
'idsite=1&rec=1&uadata[-5]=a&uadata[]=b',
'idsite=1&rec=1&uadata[2147483648]=a',
'idsite=1&rec=1&uadata[foo=a',
'idsite=1&rec=1&uadata[a][foo=b',
'idsite%00ignored=1&rec=1',
'idsite=1&rec=1&uid=a%00b',
'idsite=%26%2349%3B&rec=1',
'idsite=1&rec=%26%23x31%3B',
'idsite=1&rec=1&e_c=%26lt%3BVideo%26gt%3B&e_a=play',
'idsite=1&rec=1&uid=%26%2339%3B%22%26%26eacute%3B',
'idsite=1&rec=1&token_auth=%26%2348%3B&cip=192.0.2.1',
'idsite=1&rec=1&pdf=1&cookie[]=1',
'idsite=1&rec=1&pdf=true&cookie=1',
];
$readers=['idsite'=>[0,'integer'],'rec'=>[0,'integer'],'e_c'=>['','string'],'e_a'=>['','string'],'e_v'=>[false,'float'],'url'=>['','string'],'uid'=>['','string'],'token_auth'=>['','string'],'ec_items'=>['','json'],'uadata'=>['','json'],'pdf'=>[0,'integer'],'cookie'=>[0,'integer']];
$rows=[];
foreach($queries as $query){parse_str($query,$params); $outputs=[];foreach($readers as $name=>$spec){try{$outputs[$name]=\Piwik\Common::getRequestVar($name,$spec[0],$spec[1],$params);}catch(\Throwable $e){$outputs[$name]=['exception_class'=>get_class($e)];}}$rows[]=['query'=>$query,'parsed'=>$params,'readers'=>$outputs];}
$entities=array_flip(get_html_translation_table(HTML_ENTITIES, ENT_QUOTES | ENT_HTML401, 'UTF-8'));
$numeric=[];for($i=0;$i<160;$i++) {$input='&#'.$i.';';$numeric[]=['codepoint'=>$i,'decoded'=>html_entity_decode($input,ENT_QUOTES,'UTF-8'),'sanitized'=>\Piwik\Common::sanitizeInputValues($input)];}
foreach([0xD7FF,0xD800,0xDFFF,0xE000,0xFFFE,0xFFFF,0x10000,0x10FFFF,0x110000] as $i){$input='&#'.$i.';';$numeric[]=['codepoint'=>$i,'decoded'=>html_entity_decode($input,ENT_QUOTES,'UTF-8'),'sanitized'=>\Piwik\Common::sanitizeInputValues($input)];}
$native=[];foreach(['idsite'=>['&#49;','&amp;#49;','1'.chr(0),'&plus;1','&#x31;'],'uid'=>['<>&\"\'',chr(0).'a'.chr(0), '&copy; &apos; &notanentity;','😀'],'pdf'=>[true,false,[],1,'&#49;']] as $name=>$inputs){foreach($inputs as $input){$spec=$readers[$name];$native[]=['name'=>$name,'input'=>$input,'output'=>\Piwik\Common::getRequestVar($name,$spec[0],$spec[1],[$name=>$input])];}}
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,'arg_separator_input'=>ini_get('arg_separator.input'),'max_input_vars'=>ini_get('max_input_vars'),'max_input_nesting_level'=>ini_get('max_input_nesting_level'),'precision'=>ini_get('precision'),'source_sha256'=>hash_file('sha256','/source/Common.php'),'rows'=>$rows,'entities'=>$entities,'numeric_entities'=>$numeric,'native'=>$native],JSON_PRETTY_PRINT|JSON_INVALID_UTF8_SUBSTITUTE|JSON_UNESCAPED_SLASHES|JSON_UNESCAPED_UNICODE)."\n";
