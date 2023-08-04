package com.tomcat.service.impl;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.domain.User;
import com.tomcat.domain.UserRepository;
import com.tomcat.service.UserService;
import com.tomcat.utils.JwtUtil;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import com.tomcat.utils.SecurityUtils;

@Service
public class UserServiceImpl implements UserService {

  @Autowired
  private UserRepository userRepository;
  

  @Override
  public AuthenticationResponse register(AuthenticationRequest request) {

    // 密码编码
    String encodedPassword = SecurityUtils.encodePassword(request.password);
    
    User user = new User();
    user.setUsername(request.username);
    user.setPassword(encodedPassword);
    user.setEmail(request.email);
    user.setPhoneNum(request.phoneNum);
    user.addDeviceId(request.deviceId);

    User userSaved = userRepository.save(user);
    String jwtToken = JwtUtil.sign(userSaved.getId(), userSaved.getUsername());
    return new AuthenticationResponse(jwtToken);

  }

  @Override
  public AuthenticationResponse login(AuthenticationRequest request) {
    User user = userRepository.findByPhoneNumOrEmail(request.phoneNum, request.email)
        .orElseThrow(() -> new RuntimeException("用户不存在"));
    
    // 校验密码
    if (!SecurityUtils.matchesPassword(request.password, user.getPassword())) {
      throw new RuntimeException("密码不正确");
    }
   
    // 返回JWT令牌
    String jwtToken = JwtUtil.sign(user.getId(), user.getUsername());
    return new AuthenticationResponse(jwtToken);
  }

}