package com.tomcat.websocket;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.ChatResp;
import com.tomcat.controller.response.Lesson;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.service.AiCTutor;
import com.tomcat.utils.JwtUtil;
import com.unfbx.chatgpt.entity.chat.ChatCompletion;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

import java.util.ArrayList;
import java.util.List;

@Slf4j
public class MessageProcessor {

    AiCTutor aiClient;
    Session session;

    String uid;




    public MessageProcessor(AiCTutor aiClient, Session session, String uid){
        this.aiClient = aiClient;
        this.session = session;
        this.uid = uid;
    }

    private void chatInit(){
        ChatResp<String> resp = aiClient.chatInit(uid);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));
    }

    private void curriculumPlan(ChatReq req){
        ChatResp<List<Lesson>> resp = aiClient.curriculumPlan(req);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));

    }


    public void processor(String msg) {

//        String key = msg.split(":")[0];
//        String jsonStr = msg.split(":")[1];
//
//        if(StrUtil.equals("command", key)){
//            switch (jsonStr.toUpperCase()){
//                case Command.CHAT_INIT:
//                    this.chatInit();
//                    break;
//                case Command.CURRICULUM_PLAN:
//                    ChatReq req = new ChatReq();
//                    req.setUid(uid);
//                    req.setCommand(Command.CURRICULUM_PLAN);
//                    this.curriculumPlan(req);
//                    break;
//                default:
//                    break;
//            }
//        }



        ChatReq chatReq = JSONUtil.toBean(msg, ChatReq.class);
        chatReq.setUid(uid);
        switch (chatReq.getCommand().toUpperCase()){
            case Command.CHAT_INIT:
                this.chatInit();
                break;
            case Command.CURRICULUM_PLAN:
                this.curriculumPlan(chatReq);
                break;
            default:
                break;
        }



        //TODO
        //        检查缓存中是否存在上下文
        //        请求openai API 获得返回数据
        //        推送消息到订阅用户的设备上
        //        异步生成消息摘要用于搜索
//        String uid = session.getAttribute(JwtUtil.KEY_USER_ID);
//        log.info("processor userId:" + uid);
//        String messageContext = (String) LocalCache.CACHE.get(uid);
//        List<Message> messages = new ArrayList<>();
//        if (StrUtil.isNotBlank(messageContext)) {
//            messages = JSONUtil.toList(messageContext, Message.class);
//            if (messages.size() >= 10) {
//                messages = messages.subList(1, 10);
//            }
//            Message currentMessage = Message.builder().content(msg).role(Message.Role.USER).build();
//            messages.add(currentMessage);
//        } else {
//            Message currentMessage = Message.builder().content(msg).role(Message.Role.USER).build();
//            messages.add(currentMessage);
//        }
//        ChatCompletionResponse response = aiClient.chatCompletion(messages);
//        Message responseMag = response.getChoices().get(0).getMessage();
//        session.sendText(new TextWebSocketFrame("服务器消息 " + uid + "：" + JSONUtil.toJsonStr(responseMag)));
//        LocalCache.CACHE.put(uid, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
    }



}
